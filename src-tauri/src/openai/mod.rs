mod client;
mod responses;
mod sse;
mod stream;

pub use agent_core::{
    extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools,
};
pub use client::OpenAiClient;
pub use responses::{create_response, ResponsesCreateRequest};
pub use stream::{stream_chat_completion, ChatMessageInput};
