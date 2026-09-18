use std::collections::HashMap;
use std::io::Read;

use flate2::read::GzDecoder;

use agent_core::GithubCodingError;

/// Extract text files from a `.tar.gz` GitHub tarball (ustar, 512-byte blocks).
pub fn extract_tarball_text_files(
    archive_bytes: &[u8],
) -> Result<HashMap<String, Vec<u8>>, GithubCodingError> {
    let decoder = GzDecoder::new(archive_bytes);
    let mut tar = decoder;
    let mut files = HashMap::new();
    let mut header = [0u8; 512];
    while read_exact(&mut tar, &mut header)? {
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let name = parse_tar_name(&header)?;
        let size = parse_tar_size(&header)?;
        if size == 0 {
            continue;
        }
        let mut payload = vec![0u8; size as usize];
        read_exact(&mut tar, &mut payload)?;
        let padding = (512 - (size as usize % 512)) % 512;
        if padding > 0 {
            let mut pad = vec![0u8; padding];
            read_exact(&mut tar, &mut pad)?;
        }
        if let Some(relative) = strip_archive_root(&name) {
            if std::str::from_utf8(&payload).is_ok() {
                files.insert(relative, payload);
            }
        }
    }
    if files.is_empty() {
        return Err(GithubCodingError::Provider(
            "repository archive contained no text files".into(),
        ));
    }
    Ok(files)
}

fn read_exact<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool, GithubCodingError> {
    let mut offset = 0;
    while offset < buf.len() {
        let read = reader
            .read(&mut buf[offset..])
            .map_err(|e| GithubCodingError::Provider(e.to_string()))?;
        if read == 0 {
            return Ok(false);
        }
        offset += read;
    }
    Ok(true)
}

fn parse_tar_name(header: &[u8; 512]) -> Result<String, GithubCodingError> {
    let end = header
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(header.len());
    Ok(String::from_utf8_lossy(&header[..end]).to_string())
}

fn parse_tar_size(header: &[u8; 512]) -> Result<u64, GithubCodingError> {
    let slice = &header[124..136];
    let trimmed = slice
        .iter()
        .map(|b| *b as char)
        .collect::<String>()
        .trim()
        .to_string();
    if trimmed.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(&trimmed, 8).map_err(|e| GithubCodingError::Provider(e.to_string()))
}

fn strip_archive_root(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[1..].join("/"))
}
