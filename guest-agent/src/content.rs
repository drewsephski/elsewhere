//! File-content encoding for the guest JSON protocol.
//!
//! `encoding: "base64"` is binary-safe. Omitting it preserves the original UTF-8
//! string payload for older hosts.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::Value;

pub const ENCODING_BASE64: &str = "base64";

pub fn decode_write_content(params: &Value) -> Result<Vec<u8>, String> {
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "write_file requires path and content".to_string())?;
    match params.get("encoding").and_then(|v| v.as_str()) {
        Some(ENCODING_BASE64) => BASE64
            .decode(content.trim().as_bytes())
            .map_err(|e| format!("invalid base64 content: {e}")),
        _ => Ok(content.as_bytes().to_vec()),
    }
}

pub fn encode_read_content(bytes: &[u8], encoding: Option<&str>) -> Result<String, String> {
    if encoding == Some(ENCODING_BASE64) {
        return Ok(BASE64.encode(bytes));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        "file is not valid UTF-8; request encoding=base64 for binary files".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn base64_roundtrip_includes_non_utf8() {
        let original = vec![0u8, 255, 10, 0, 128];
        let params = json!({
            "content": BASE64.encode(&original),
            "encoding": ENCODING_BASE64,
        });
        assert_eq!(decode_write_content(&params).unwrap(), original);
        assert_eq!(
            encode_read_content(&original, Some(ENCODING_BASE64)).unwrap(),
            BASE64.encode(&original)
        );
    }

    #[test]
    fn utf8_legacy_write_still_works() {
        let params = json!({ "content": "hello" });
        assert_eq!(decode_write_content(&params).unwrap(), b"hello");
        assert_eq!(encode_read_content(b"hello", None).unwrap(), "hello");
    }
}
