use crate::error::AppError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

/// Incremental SSE decoder that preserves UTF-8 across arbitrary byte chunk boundaries.
pub struct SseDecoder {
    byte_buffer: Vec<u8>,
    event_buffer: String,
    saw_done: bool,
    saw_finish: bool,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self {
            byte_buffer: Vec::new(),
            event_buffer: String::new(),
            saw_done: false,
            saw_finish: false,
        }
    }

    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Vec<String>, AppError> {
        self.byte_buffer.extend_from_slice(chunk);
        self.drain_valid_utf8()?;
        self.collect_content_deltas()
    }

    pub fn finish(&self) -> Result<(), AppError> {
        if self.saw_done || self.saw_finish {
            return Ok(());
        }
        if !self.byte_buffer.is_empty() || !self.event_buffer.is_empty() {
            return Err(AppError::Provider(
                "stream ended before completion marker".into(),
            ));
        }
        Err(AppError::Provider(
            "stream ended before completion marker".into(),
        ))
    }

    fn drain_valid_utf8(&mut self) -> Result<(), AppError> {
        loop {
            if self.byte_buffer.is_empty() {
                return Ok(());
            }
            match std::str::from_utf8(&self.byte_buffer) {
                Ok(text) => {
                    self.event_buffer.push_str(text);
                    self.byte_buffer.clear();
                    return Ok(());
                }
                Err(err) => {
                    let valid_up_to = err.valid_up_to();
                    if valid_up_to > 0 {
                        let valid = std::str::from_utf8(&self.byte_buffer[..valid_up_to])
                            .map_err(|e| AppError::Provider(format!("invalid utf-8: {}", e)))?;
                        self.event_buffer.push_str(valid);
                        self.byte_buffer.drain(..valid_up_to);
                    }
                    match err.error_len() {
                        Some(_) => {
                            return Err(AppError::Provider("invalid utf-8 in stream".into()));
                        }
                        None => return Ok(()),
                    }
                }
            }
        }
    }

    fn collect_content_deltas(&mut self) -> Result<Vec<String>, AppError> {
        let mut deltas = Vec::new();

        while let Some(pos) = self.event_buffer.find("\n\n") {
            let block = self.event_buffer[..pos].to_string();
            self.event_buffer = self.event_buffer[pos + 2..].to_string();

            for line in block.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = line.strip_prefix("data: ").unwrap_or("");
                if data == "[DONE]" {
                    self.saw_done = true;
                    continue;
                }
                let parsed: StreamChunk = serde_json::from_str(data)
                    .map_err(|e| AppError::Provider(format!("malformed stream chunk: {}", e)))?;
                for choice in parsed.choices {
                    if choice.finish_reason.is_some() {
                        self.saw_finish = true;
                    }
                    if let Some(content) = choice.delta.content {
                        if !content.is_empty() {
                            deltas.push(content);
                        }
                    }
                }
            }
        }

        Ok(deltas)
    }
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_multibyte_utf8_across_chunks() {
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"🌍\"}}]}\n\n";
        let bytes = payload.as_bytes();
        let split = bytes.len() / 2;

        let mut decoder = SseDecoder::new();
        let _ = decoder.push_chunk(&bytes[..split]).expect("part a");
        let deltas = decoder.push_chunk(&bytes[split..]).expect("part b");
        assert_eq!(deltas.join(""), "🌍");
        decoder.push_chunk(b"data: [DONE]\n\n").expect("done");
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn truncated_stream_without_done_fails() {
        let mut decoder = SseDecoder::new();
        let chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n";
        decoder.push_chunk(chunk.as_bytes()).expect("chunk");
        let err = decoder.finish().unwrap_err();
        assert!(err.to_string().contains("completion marker"));
    }

    #[test]
    fn done_marker_allows_finish() {
        let mut decoder = SseDecoder::new();
        decoder
            .push_chunk(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
            )
            .expect("chunk");
        assert!(decoder.finish().is_ok());
    }
}
