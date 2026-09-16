//! Bounded document text extraction. No OCR. Attachment bytes stay data, never instructions.

use agent_core::{
    AttachmentKind, AttachmentTextPage, MAX_ATTACHMENT_READ_BYTES, MAX_INLINE_DOCUMENT_BYTES,
    UNTRUSTED_DOCUMENT_PREFACE,
};

const PDF_EXTRACT_SCAN_BYTES: usize = 2 * 1024 * 1024;

pub fn extract_text_page(
    kind: AttachmentKind,
    mime: &str,
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> AttachmentTextPage {
    let extracted = extract_all(kind, mime, bytes);
    let limit = limit.clamp(1, MAX_ATTACHMENT_READ_BYTES);
    let start = offset.min(extracted.len());
    let end = (start + limit).min(extracted.len());
    let next = if end < extracted.len() {
        Some(end)
    } else {
        None
    };
    AttachmentTextPage {
        attachment_id: String::new(),
        text: extracted[start..end].to_string(),
        offset: start,
        next_offset: next,
        truncated: end < extracted.len(),
        extraction_available: !extracted.is_empty() && extracted != unscanned_pdf_placeholder(),
    }
}

pub fn inline_document_excerpt(kind: AttachmentKind, mime: &str, bytes: &[u8]) -> String {
    let page = extract_text_page(kind, mime, bytes, 0, MAX_INLINE_DOCUMENT_BYTES);
    if !page.extraction_available {
        return format!(
            "{UNTRUSTED_DOCUMENT_PREFACE}\n{}",
            unscanned_pdf_placeholder()
        );
    }
    let mut body = page.text;
    if page.truncated {
        body.push_str("\n[truncated; use attachment_read for more]");
    }
    format!("{UNTRUSTED_DOCUMENT_PREFACE}\n{body}")
}

fn extract_all(kind: AttachmentKind, mime: &str, bytes: &[u8]) -> String {
    match kind {
        AttachmentKind::Text => extract_text(mime, bytes),
        AttachmentKind::Pdf => extract_pdf(bytes),
        AttachmentKind::Image => String::new(),
    }
}

fn extract_text(mime: &str, bytes: &[u8]) -> String {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return "This text attachment is not valid UTF-8.".into();
    };
    if mime == "application/json" {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            return serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string());
        }
    }
    text.to_string()
}

fn extract_pdf(bytes: &[u8]) -> String {
    let scan = &bytes[..bytes.len().min(PDF_EXTRACT_SCAN_BYTES)];
    let mut chunks = Vec::new();
    extract_pdf_literal_strings(scan, &mut chunks);
    extract_pdf_flate_streams(scan, &mut chunks);
    let joined = chunks.join("\n");
    let cleaned = joined
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect::<String>();
    let trimmed = collapse_pdf_whitespace(&cleaned);
    if trimmed.chars().filter(|c| c.is_alphanumeric()).count() < 8 {
        return unscanned_pdf_placeholder().into();
    }
    trimmed
}

fn unscanned_pdf_placeholder() -> &'static str {
    "This PDF could not be text-extracted. It may be scanned or image-only. OCR is not available in this version."
}

fn extract_pdf_literal_strings(bytes: &[u8], out: &mut Vec<String>) {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let mut s = String::new();
            i += 1;
            while i < bytes.len() && bytes[i] != b')' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i].is_ascii_graphic() || bytes[i] == b' ' {
                    s.push(bytes[i] as char);
                }
                i += 1;
                if s.len() > 4_096 {
                    break;
                }
            }
            if s.len() >= 4 {
                out.push(s);
            }
        } else {
            i += 1;
        }
    }
}

fn extract_pdf_flate_streams(bytes: &[u8], out: &mut Vec<String>) {
    let marker = b"/FlateDecode";
    let mut search = 0;
    while let Some(rel) = find_slice(&bytes[search..], marker) {
        let start = search + rel;
        if let Some(stream_rel) = find_slice(&bytes[start..], b"stream") {
            let stream_start = start + stream_rel + "stream".len();
            let data_start = skip_pdf_newline(bytes, stream_start);
            if let Some(end_rel) = find_slice(&bytes[data_start..], b"endstream") {
                let compressed = &bytes[data_start..data_start + end_rel];
                if let Ok(decoded) = inflate_pdf(compressed) {
                    extract_pdf_literal_strings(&decoded, out);
                    if let Ok(text) = std::str::from_utf8(&decoded) {
                        let printable: String = text
                            .chars()
                            .filter(|c| c.is_ascii_graphic() || matches!(c, ' ' | '\n'))
                            .collect();
                        if printable.len() > 16 {
                            out.push(printable);
                        }
                    }
                }
                search = data_start + end_rel + 9;
                continue;
            }
        }
        search = start + marker.len();
    }
}

fn inflate_pdf(data: &[u8]) -> Result<Vec<u8>, ()> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|_| ())?;
    Ok(out)
}

fn skip_pdf_newline(bytes: &[u8], mut i: usize) -> usize {
    if i < bytes.len() && bytes[i] == b'\r' {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'\n' {
        i += 1;
    }
    i
}

fn find_slice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn collapse_pdf_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
        if out.len() > MAX_ATTACHMENT_READ_BYTES * 4 {
            break;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_untrusted_text() {
        let excerpt = inline_document_excerpt(AttachmentKind::Text, "text/plain", b"hello vendor");
        assert!(excerpt.contains("untrusted user-provided document content"));
        assert!(excerpt.contains("hello vendor"));
    }

    #[test]
    fn scanned_pdf_is_honest() {
        let text = extract_pdf(b"%PDF-1.4 binary junk \x00\x01\x02");
        assert!(text.contains("could not be text-extracted"));
    }
}
