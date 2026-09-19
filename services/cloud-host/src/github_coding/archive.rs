use std::collections::HashMap;
use std::io::Read;

use agent_core::GithubCodingError;
use flate2::read::GzDecoder;
use tar::EntryType;

/// Maximum compressed tarball download size (host will reject larger).
pub const MAX_COMPRESSED_TARBALL_BYTES: usize = 64 * 1024 * 1024;
/// Maximum total decompressed bytes from a single archive.
pub const MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
/// Maximum single file size inside the archive.
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum number of regular files extracted.
pub const MAX_FILE_COUNT: usize = 10_000;

#[derive(Debug, Clone)]
pub struct ArchiveFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
    pub mode: u32,
}

pub fn extract_tarball_files(
    archive_bytes: &[u8],
) -> Result<Vec<ArchiveFile>, GithubCodingError> {
    if archive_bytes.len() > MAX_COMPRESSED_TARBALL_BYTES {
        return Err(GithubCodingError::Validation(
            "repository archive exceeds maximum download size".into(),
        ));
    }
    let decoder = GzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(decoder);
    let mut files = Vec::new();
    let mut decompressed_total = 0usize;
    for entry in archive.entries().map_err(map_tar_err)? {
        let mut entry = entry.map_err(map_tar_err)?;
        let header = entry.header();
        let raw_path = header.path().map_err(map_tar_err)?;
        let raw_path_str = raw_path.to_string_lossy();
        if raw_path_str.is_empty() {
            continue;
        }
        if raw_path.is_absolute()
            || raw_path_str.contains("..")
            || raw_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(GithubCodingError::Validation(
                "repository archive contains unsafe paths".into(),
            ));
        }
        let path = entry.path().map_err(map_tar_err)?.into_owned();
        let path_str = path.to_string_lossy();
        let entry_type = header.entry_type();
        if entry_type == EntryType::Symlink || entry_type == EntryType::Link {
            return Err(GithubCodingError::Validation(
                "repository archive symlinks are not supported in this slice".into(),
            ));
        }
        if !entry_type.is_file() {
            continue;
        }
        let size = header.size().map_err(map_tar_err)? as usize;
        let mode = header.mode().map_err(map_tar_err)?;
        if size > MAX_FILE_BYTES {
            return Err(GithubCodingError::Validation(
                "repository archive file exceeds maximum size".into(),
            ));
        }
        decompressed_total = decompressed_total.saturating_add(size);
        if decompressed_total > MAX_DECOMPRESSED_BYTES {
            return Err(GithubCodingError::Validation(
                "repository archive exceeds maximum decompressed size".into(),
            ));
        }
        if files.len() >= MAX_FILE_COUNT {
            return Err(GithubCodingError::Validation(
                "repository archive contains too many files".into(),
            ));
        }
        let relative = strip_github_archive_root(&path_str)?;
        let mut payload = Vec::with_capacity(size);
        entry.read_to_end(&mut payload).map_err(map_tar_err)?;
        files.push(ArchiveFile {
            relative_path: relative,
            bytes: payload,
            mode,
        });
    }
    if files.is_empty() {
        return Err(GithubCodingError::Provider(
            "repository archive contained no files".into(),
        ));
    }
    Ok(files)
}

pub fn files_to_map(files: &[ArchiveFile]) -> HashMap<String, ArchiveFile> {
    let mut map = HashMap::new();
    for file in files {
        map.insert(file.relative_path.clone(), file.clone());
    }
    map
}

fn strip_github_archive_root(path: &str) -> Result<String, GithubCodingError> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return Err(GithubCodingError::Provider(
            "unexpected archive entry path".into(),
        ));
    }
    Ok(parts[1..].join("/"))
}

fn map_tar_err(err: std::io::Error) -> GithubCodingError {
    GithubCodingError::Provider(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn build_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            for (name, data) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o100644);
                header.set_entry_type(EntryType::Regular);
                header.set_path(name).unwrap();
                header.set_cksum();
                builder.append(&header, &data[..]).unwrap();
            }
            builder.finish().unwrap();
        }
        let mut gz = Vec::new();
        let mut enc = GzEncoder::new(&mut gz, Compression::default());
        enc.write_all(&tar_buf).unwrap();
        enc.finish().unwrap();
        gz
    }

    fn ustar_entry(path: &str, data: &[u8]) -> Vec<u8> {
        let mut header = [0u8; 512];
        let path_bytes = path.as_bytes();
        assert!(path_bytes.len() <= 100, "ustar path too long");
        header[..path_bytes.len()].copy_from_slice(path_bytes);
        let size_octal = format!("{:011o}\0", data.len());
        header[124..124 + 12].copy_from_slice(size_octal.as_bytes());
        header[156] = b'0';
        for i in 148..156 {
            header[i] = b' ';
        }
        let cksum: u64 = header.iter().map(|&b| u64::from(b)).sum();
        let mut cksum_field = [b' '; 8];
        let cksum_str = format!("{:06o}", cksum);
        cksum_field[..cksum_str.len()].copy_from_slice(cksum_str.as_bytes());
        header[148..156].copy_from_slice(&cksum_field);
        let mut block = Vec::with_capacity(512 + data.len() + (512 - data.len() % 512) % 512 + 512);
        block.extend_from_slice(&header);
        block.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        block.extend(vec![0u8; pad]);
        block.extend([0u8; 512]);
        block
    }

    #[test]
    fn rejects_parent_traversal() {
        let body = ustar_entry("repo-main/../secret", b"x");
        let mut gz = Vec::new();
        let mut enc = flate2::write::GzEncoder::new(&mut gz, Compression::default());
        enc.write_all(&body).unwrap();
        enc.finish().unwrap();
        let err = extract_tarball_files(&gz).expect_err("traversal");
        assert!(
            matches!(err, GithubCodingError::Validation(_)),
            "expected validation error for traversal, got {:?}",
            err
        );
    }

    #[test]
    fn extracts_regular_file() {
        let archive = build_tar_gz(&[("repo-main/README.md", b"Hello")]);
        let files = extract_tarball_files(&archive).expect("extract");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "README.md");
    }
}
