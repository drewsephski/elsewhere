use codex_provider::run_mcp_turn_probe;

#[tokio::main]
async fn main() {
    if std::env::var("OPENAI_API_KEY").is_ok() {
        eprintln!("OPENAI_API_KEY must be unset for mcp_turn_probe");
        std::process::exit(1);
    }

    let model = std::env::var("ELSEWHERE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.6-luna".into());

    match run_mcp_turn_probe(&model).await {
        Ok(result) => {
            println!("MCP turn probe passed.");
            println!("Assistant: {}", result.assistant_text.trim());
            println!(
                "MCP write/read completed: write={} read={}",
                result.mcp_write_completed, result.mcp_read_completed
            );
        }
        Err(err) => {
            eprintln!("MCP turn probe failed: {err}");
            std::process::exit(1);
        }
    }
}
