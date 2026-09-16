//! Current-run attachment reads for the parent Bot (not AgentComputer).

use async_trait::async_trait;

use crate::run_user_input::AttachmentDescriptor;

pub const MAX_ATTACHMENT_READ_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentError {
    NotFound,
    Validation(String),
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTextPage {
    pub attachment_id: String,
    pub text: String,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub truncated: bool,
    pub extraction_available: bool,
}

#[async_trait]
pub trait AgentAttachments: Send + Sync {
    async fn list_for_run(&self) -> Result<Vec<AttachmentDescriptor>, AttachmentError>;

    async fn read_text(
        &self,
        attachment_id: &str,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<AttachmentTextPage, AttachmentError>;

    /// Host-side image bytes for native model input. Never returned to the model as base64.
    async fn load_image_bytes(
        &self,
        attachment_id: &str,
    ) -> Result<(AttachmentDescriptor, Vec<u8>), AttachmentError>;
}

#[derive(Default)]
pub struct InMemoryAttachments {
    items: std::sync::Mutex<Vec<(AttachmentDescriptor, Vec<u8>)>>,
}

impl InMemoryAttachments {
    pub fn new(items: Vec<(AttachmentDescriptor, Vec<u8>)>) -> Self {
        Self {
            items: std::sync::Mutex::new(items),
        }
    }
}

#[async_trait]
impl AgentAttachments for InMemoryAttachments {
    async fn list_for_run(&self) -> Result<Vec<AttachmentDescriptor>, AttachmentError> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .map(|(descriptor, _)| descriptor.clone())
            .collect())
    }

    async fn read_text(
        &self,
        attachment_id: &str,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<AttachmentTextPage, AttachmentError> {
        let items = self.items.lock().unwrap();
        let (descriptor, bytes) = items
            .iter()
            .find(|(item, _)| item.id == attachment_id)
            .ok_or(AttachmentError::NotFound)?;
        if !descriptor.kind.textual_extraction_available() {
            return Err(AttachmentError::Validation(
                "That attachment has no extracted text.".into(),
            ));
        }
        let text = String::from_utf8_lossy(bytes).to_string();
        let start = offset.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(MAX_ATTACHMENT_READ_BYTES as u64) as usize;
        let start = start.min(text.len());
        let end = (start + limit).min(text.len());
        Ok(AttachmentTextPage {
            attachment_id: descriptor.id.clone(),
            text: text[start..end].to_string(),
            offset: start,
            next_offset: if end < text.len() { Some(end) } else { None },
            truncated: end < text.len(),
            extraction_available: true,
        })
    }

    async fn load_image_bytes(
        &self,
        attachment_id: &str,
    ) -> Result<(AttachmentDescriptor, Vec<u8>), AttachmentError> {
        let items = self.items.lock().unwrap();
        let (descriptor, bytes) = items
            .iter()
            .find(|(item, _)| item.id == attachment_id)
            .ok_or(AttachmentError::NotFound)?;
        if !descriptor.kind.is_image() {
            return Err(AttachmentError::Validation(
                "That attachment is not an image".into(),
            ));
        }
        Ok((descriptor.clone(), bytes.clone()))
    }
}
