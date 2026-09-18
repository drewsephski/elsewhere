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
        let path = entry
            .path()
            .map_err(map_tar_err)?
            .into_owned();
        let path_str = path.to_string_lossy();
        if path_str.is_empty() {
            continue;
        }
        if path.is_absolute() || path_str.contains("..") {
            return Err(GithubCodingError::Validation(
                "repository archive contains unsafe paths".into(),
            ));
        }
        let header = entry.header();
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

    #[test]
    fn rejects_parent_traversal() {
        let archive = build_tar_gz(&[("repo-main/../secret", b"x")]);
        let err = extract_tarball_files(&archive).expect_err("traversal");
        assert!(matches!(err, GithubCodingError::Validation(_)));
    }

    #[test]
    fn extracts_regular_file() {
        let archive = build_tar_gz(&[("repo-main/README.md", b"Hello")]);
        let files = extract_tarball_files(&archive).expect("extract");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "README.md");
    }
}
