//! `messages.kind` separates user-visible chat rows from structured runner artifacts.

pub const CHAT_MESSAGE_KIND: &str = "chat";

#[inline]
pub fn is_transcript_visible_kind(kind: &str) -> bool {
    kind == CHAT_MESSAGE_KIND
}
