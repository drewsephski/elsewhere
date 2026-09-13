mod client;
mod stream;

pub use client::OpenAiClient;
pub use stream::{assemble_stream_chunks, stream_chat_completion, ChatMessageInput};
