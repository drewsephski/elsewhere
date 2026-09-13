mod client;
mod stream;

pub use client::OpenAiClient;
pub use stream::{stream_chat_completion, ChatMessageInput};
