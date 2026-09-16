pub mod api;
pub mod extract;
pub mod store;
pub mod validate;

use std::sync::Arc;

use agent_core::{
    AgentAttachments, AgentComputer, AttachmentDescriptor, AttachmentError, AttachmentKind,
    AttachmentTextPage, ComputerError, MAX_ATTACHMENT_READ_BYTES,
};
use async_trait::async_trait;
use sqlx::PgPool;

use crate::error::ApiError;

#[derive(Clone)]
pub struct RunScopedAttachments {
    pool: PgPool,
    run_id: String,
}

impl RunScopedAttachments {
    pub fn new(pool: PgPool, run_id: impl Into<String>) -> Arc<dyn AgentAttachments> {
        Arc::new(Self {
            pool,
            run_id: run_id.into(),
        })
    }
}

#[async_trait]
impl AgentAttachments for RunScopedAttachments {
    async fn list_for_run(&self) -> Result<Vec<AttachmentDescriptor>, AttachmentError> {
        store::list_for_run(&self.pool, &self.run_id)
            .await
            .map_err(|e| AttachmentError::Internal(e.to_string()))
    }

    async fn read_text(
        &self,
        attachment_id: &str,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<AttachmentTextPage, AttachmentError> {
        let (descriptor, bytes) =
            store::load_run_attachment_bytes(&self.pool, &self.run_id, attachment_id)
                .await
                .map_err(|e| AttachmentError::Internal(e.to_string()))?
                .ok_or(AttachmentError::NotFound)?;
        if !descriptor.kind.textual_extraction_available() {
            return Err(AttachmentError::Validation(
                "That attachment has no extracted text. Images are provided as native model input."
                    .into(),
            ));
        }
        let mut page = extract::extract_text_page(
            descriptor.kind,
            &descriptor.mime_type,
            &bytes,
            offset.unwrap_or(0) as usize,
            limit.unwrap_or(MAX_ATTACHMENT_READ_BYTES as u64) as usize,
        );
        page.attachment_id = descriptor.id;
        Ok(page)
    }

    async fn load_image_bytes(
        &self,
        attachment_id: &str,
    ) -> Result<(AttachmentDescriptor, Vec<u8>), AttachmentError> {
        let (descriptor, bytes) =
            store::load_run_attachment_bytes(&self.pool, &self.run_id, attachment_id)
                .await
                .map_err(|e| AttachmentError::Internal(e.to_string()))?
                .ok_or(AttachmentError::NotFound)?;
        if !descriptor.kind.is_image() {
            return Err(AttachmentError::Validation(
                "That attachment is not an image".into(),
            ));
        }
        Ok((descriptor, bytes))
    }
}

pub async fn stage_run_attachments_into_workspace(
    computer: &dyn AgentComputer,
    pool: &PgPool,
    run_id: &str,
) -> Result<Vec<AttachmentDescriptor>, ApiError> {
    let descriptors = store::list_for_run(pool, run_id).await?;
    for descriptor in &descriptors {
        if descriptor.workspace_path.contains("..")
            || !descriptor
                .workspace_path
                .starts_with(&format!("/workspace/inputs/{run_id}/"))
        {
            return Err(ApiError::Internal(
                "attachment workspace path failed ownership validation".into(),
            ));
        }
        let (_, bytes) = store::load_run_attachment_bytes(pool, run_id, &descriptor.id)
            .await?
            .ok_or_else(|| ApiError::Internal("run attachment bytes missing".into()))?;
        computer
            .write_file(&descriptor.workspace_path, &bytes)
            .await
            .map_err(map_computer_error)?;
    }
    Ok(descriptors)
}

fn map_computer_error(err: ComputerError) -> ApiError {
    ApiError::Internal(err.to_string())
}

pub fn compose_user_text_with_documents(
    user_text: &str,
    descriptors: &[AttachmentDescriptor],
    extracts: &[(String, String)],
) -> String {
    let mut out = user_text.trim().to_string();
    if descriptors.is_empty() {
        return out;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(agent_core::ATTACHMENT_SAFETY_CONTRACT);
    out.push('\n');
    for descriptor in descriptors {
        out.push_str(&format!(
            "\nAttachment {} ({}, {}, {} bytes) staged at {}.\n",
            descriptor.id,
            descriptor.original_name,
            descriptor.mime_type,
            descriptor.size_bytes,
            descriptor.workspace_path
        ));
        if descriptor.kind == AttachmentKind::Image {
            out.push_str("This image is also supplied as native model image input.\n");
            continue;
        }
        if let Some((_, excerpt)) = extracts.iter().find(|(id, _)| id == &descriptor.id) {
            out.push_str(excerpt);
            out.push('\n');
        }
    }
    out
}

pub const UPLOAD_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;

pub async fn responses_input_with_images(
    mut input_messages: Vec<serde_json::Value>,
    pool: &PgPool,
    run_id: &str,
    descriptors: &[AttachmentDescriptor],
) -> Result<Vec<serde_json::Value>, ApiError> {
    let images: Vec<_> = descriptors
        .iter()
        .filter(|d| d.kind.is_image())
        .cloned()
        .collect();
    if images.is_empty() {
        return Ok(input_messages);
    }
    let Some(last) = input_messages.last_mut() else {
        return Ok(input_messages);
    };
    if last.get("role").and_then(|v| v.as_str()) != Some("user") {
        return Ok(input_messages);
    }
    let text = last
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut parts = vec![serde_json::json!({"type": "input_text", "text": text})];
    for descriptor in images {
        let Some((_, bytes)) =
            store::load_run_attachment_bytes(pool, run_id, &descriptor.id).await?
        else {
            continue;
        };
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        parts.push(serde_json::json!({
            "type": "input_image",
            "image_url": format!("data:{};base64,{}", descriptor.mime_type, b64)
        }));
    }
    last["content"] = serde_json::Value::Array(parts);
    Ok(input_messages)
}
