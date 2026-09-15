//! UTF-8-safe bounded text helpers (byte limits).

/// Truncate `text` to at most `max_bytes` UTF-8 bytes without splitting code points.
/// Appends U+2026 ellipsis only when truncated.
pub fn truncate_utf8_bytes(text: &str, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let ellipsis = "…";
    let ellipsis_len = ellipsis.len();
    if max_bytes <= ellipsis_len {
        return ellipsis.chars().take(1).collect();
    }
    let budget = max_bytes - ellipsis_len;
    let mut end = budget;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        return ellipsis.to_string();
    }
    format!("{}{}", &text[..end], ellipsis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_split_multibyte_char() {
        let s = "你好世界";
        let out = truncate_utf8_bytes(s, 7);
        assert!(out.ends_with('…'));
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn short_text_unchanged() {
        assert_eq!(truncate_utf8_bytes("hello", 1024), "hello");
    }

    #[test]
    fn emoji_boundary() {
        let s = "a🎉b";
        let out = truncate_utf8_bytes(s, 5);
        assert_eq!(out, "a…");
    }
}
