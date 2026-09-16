//! Server-side attachment byte validation. Filename, extension, and browser MIME are untrusted.

use agent_core::{AttachmentKind, MAX_ATTACHMENT_BYTES};

const MAX_SAFE_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedUpload {
    pub original_name: String,
    pub safe_name: String,
    pub mime_type: String,
    pub kind: AttachmentKind,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadValidationError {
    Empty,
    TooLarge,
    Unsupported,
    UnsafeFilename,
    Spoofed,
}

impl UploadValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Empty => "File is empty",
            Self::TooLarge => "File exceeds the 8 MiB limit",
            Self::Unsupported => "That file type is not supported",
            Self::UnsafeFilename => "Filename is not allowed",
            Self::Spoofed => "File contents do not match a supported type",
        }
    }
}

pub fn validate_upload_bytes(
    original_name: &str,
    claimed_mime: Option<&str>,
    bytes: &[u8],
) -> Result<ValidatedUpload, UploadValidationError> {
    if bytes.is_empty() {
        return Err(UploadValidationError::Empty);
    }
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(UploadValidationError::TooLarge);
    }
    reject_executables_and_archives(bytes)?;

    let detected = detect_type(bytes).ok_or(UploadValidationError::Spoofed)?;
    if let Some(claimed) = claimed_mime
        .map(normalize_claimed_mime)
        .filter(|s| !s.is_empty())
    {
        if claimed != detected.mime && !compatible_claim(claimed, detected.mime) {
            return Err(UploadValidationError::Spoofed);
        }
    }

    let original_name = original_name.trim();
    if original_name.is_empty() {
        return Err(UploadValidationError::UnsafeFilename);
    }
    let safe_name = normalize_filename(original_name, detected.ext)?;
    let sha256 = sha256_hex(bytes);

    Ok(ValidatedUpload {
        original_name: original_name.chars().take(200).collect(),
        safe_name,
        mime_type: detected.mime.to_string(),
        kind: AttachmentKind::from_mime(detected.mime).expect("detected mime is supported"),
        size_bytes: bytes.len() as u64,
        sha256,
    })
}

struct DetectedType {
    mime: &'static str,
    ext: &'static str,
}

fn detect_type(bytes: &[u8]) -> Option<DetectedType> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(DetectedType {
            mime: "image/jpeg",
            ext: "jpg",
        });
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(DetectedType {
            mime: "image/png",
            ext: "png",
        });
    }
    if is_webp(bytes) {
        return Some(DetectedType {
            mime: "image/webp",
            ext: "webp",
        });
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(DetectedType {
            mime: "image/gif",
            ext: "gif",
        });
    }
    if bytes.starts_with(b"%PDF-") {
        return Some(DetectedType {
            mime: "application/pdf",
            ext: "pdf",
        });
    }
    if looks_like_html(bytes) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return None;
    }
    if serde_json::from_str::<serde_json::Value>(text.trim()).is_ok() {
        return Some(DetectedType {
            mime: "application/json",
            ext: "json",
        });
    }
    if looks_like_csv(text) {
        return Some(DetectedType {
            mime: "text/csv",
            ext: "csv",
        });
    }
    if looks_like_markdown(text) {
        return Some(DetectedType {
            mime: "text/markdown",
            ext: "md",
        });
    }
    Some(DetectedType {
        mime: "text/plain",
        ext: "txt",
    })
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let prefix = std::str::from_utf8(bytes.get(..64).unwrap_or(bytes))
        .unwrap_or("")
        .trim_start()
        .to_ascii_lowercase();
    prefix.starts_with("<!doctype html") || prefix.starts_with("<html")
}

fn looks_like_csv(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().take(8).collect();
    if lines.len() < 2 {
        return false;
    }
    let commas = lines.iter().filter(|line| line.contains(',')).count();
    commas >= 2
}

fn looks_like_markdown(text: &str) -> bool {
    text.contains("```")
        || text.lines().take(20).any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("# ")
                || trimmed.starts_with("## ")
                || trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
        })
}

fn reject_executables_and_archives(bytes: &[u8]) -> Result<(), UploadValidationError> {
    if bytes.starts_with(&[0x7F, b'E', b'L', b'F'])
        || bytes.starts_with(b"MZ")
        || bytes.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
        || bytes.starts_with(&[0xCE, 0xFA, 0xED, 0xFE])
        || bytes.starts_with(&[0xFE, 0xED, 0xFA, 0xCF])
        || bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"\x1f\x8b")
        || bytes.starts_with(b"Rar!")
        || bytes.starts_with(b"7z\xbc\xaf\x27\x1c")
        || bytes.starts_with(b"\x00asm")
    {
        return Err(UploadValidationError::Unsupported);
    }
    Ok(())
}

fn normalize_claimed_mime(value: &str) -> &str {
    value.split(';').next().unwrap_or(value).trim()
}

fn compatible_claim(claimed: &str, detected: &'static str) -> bool {
    match (claimed, detected) {
        ("text/plain", "text/markdown")
        | ("text/plain", "text/csv")
        | ("text/plain", "application/json") => true,
        ("text/markdown", "text/plain") => true,
        ("text/csv", "text/plain") => true,
        _ => false,
    }
}

pub fn normalize_filename(
    original: &str,
    fallback_ext: &str,
) -> Result<String, UploadValidationError> {
    let replaced = original.replace('\\', "/");
    let base = replaced.rsplit('/').next().unwrap_or(original).trim();
    if base.is_empty() || base == "." || base == ".." {
        return Err(UploadValidationError::UnsafeFilename);
    }
    if base.contains('\0') {
        return Err(UploadValidationError::UnsafeFilename);
    }
    let mut out = String::new();
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
        if out.len() >= MAX_SAFE_NAME_CHARS {
            break;
        }
    }
    out = out.trim_matches('.').to_string();
    if out.is_empty() {
        out = format!("file.{fallback_ext}");
    }
    if !out.contains('.') {
        out.push('.');
        out.push_str(fallback_ext);
    }
    if out.starts_with('.') || out.contains("..") {
        return Err(UploadValidationError::UnsafeFilename);
    }
    Ok(out)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_png_magic_and_rejects_zip() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0; 16]);
        let ok = validate_upload_bytes("chart.png", Some("image/png"), &png).unwrap();
        assert_eq!(ok.mime_type, "image/png");
        assert!(
            validate_upload_bytes("x.zip", Some("application/zip"), b"PK\x03\x04rest").is_err()
        );
    }

    #[test]
    fn rejects_html_as_text_and_spoofed_png() {
        assert!(
            validate_upload_bytes("page.txt", Some("text/plain"), b"<html><body>hi</body>")
                .is_err()
        );
        assert!(validate_upload_bytes("x.png", Some("image/png"), b"not an image").is_err());
    }

    #[test]
    fn parses_json_and_rejects_malformed() {
        let ok =
            validate_upload_bytes("data.json", Some("application/json"), br#"{"a":1}"#).unwrap();
        assert_eq!(ok.mime_type, "application/json");
        assert!(
            validate_upload_bytes("data.json", Some("application/json"), b"{nope").is_err()
                || validate_upload_bytes("data.json", Some("application/json"), b"{nope")
                    .unwrap()
                    .mime_type
                    != "application/json"
        );
    }

    #[test]
    fn sanitizes_path_filename() {
        let name = normalize_filename("../../etc/passwd.txt", "txt").unwrap();
        assert_eq!(name, "passwd.txt");
        assert!(normalize_filename("..", "txt").is_err());
    }

    #[test]
    fn rejects_oversized_file() {
        let huge = vec![b'a'; (MAX_ATTACHMENT_BYTES as usize) + 1];
        assert!(validate_upload_bytes("big.txt", Some("text/plain"), &huge).is_err());
    }
}
