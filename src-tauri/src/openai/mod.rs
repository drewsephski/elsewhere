mod client;
mod responses;
mod sse;
mod stream;

pub use client::OpenAiClient;
pub use responses::{
    create_response, extract_assistant_text, extract_function_calls, function_call_output_item,
    model_supports_responses_tools, ResponsesCreateRequest,
};
pub use stream::{stream_chat_completion, ChatMessageInput};
