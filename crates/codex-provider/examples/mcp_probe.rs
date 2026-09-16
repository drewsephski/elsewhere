use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use agent_core::{AllowAllApprovalGate, FakeAgentComputer, ToolRunContext};
use codex_provider::{
    assert_elsewhere_mcp_direct_exposure, assert_host_tools_disabled,
    build_elsewhere_thread_start_params, ensure_codex_mcp_tool_exposure_supported,
    which_codex_executable, CodexAppServerClient, CodexProcessLaunch, ElsewhereThreadConfig,
    MCP_SERVER_NAME,
};
use computer_mcp::{ComputerMcpServer, MCP_BEARER_ENV_VAR};

#[tokio::main]
async fn main() {
    let computer = Arc::new(FakeAgentComputer::new().with_listing(
        "/workspace",
        vec![agent_core::WorkspaceEntry {
            name: "hello.txt".into(),
            path: "/workspace/hello.txt".into(),
            is_dir: false,
        }],
    ));

    let run = ToolRunContext {
        run_id: "probe".into(),
        request_id: "req".into(),
        owner_id: "local".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let mcp = ComputerMcpServer::start(
        computer,
        Arc::new(AllowAllApprovalGate),
        run,
        Arc::new(AtomicBool::new(false)),
        None,
        None,
        None,
        None,
        None,
        "conv-probe".into(),
    )
    .await
    .expect("start MCP server");
    println!("MCP URL: {}", mcp.url());
    println!("MCP bearer env var: {MCP_BEARER_ENV_VAR}");

    let cwd = tempfile::tempdir().expect("temp cwd");
    let thread_config = ElsewhereThreadConfig {
        cwd: cwd.path().to_path_buf(),
        mcp_url: mcp.url().to_string(),
        bearer_env_var: MCP_BEARER_ENV_VAR.to_string(),
        model: "gpt-5.6-luna".into(),
        base_instructions: None,
        developer_instructions: None,
    };
    let thread_params = build_elsewhere_thread_start_params(&thread_config).expect("thread params");
    assert_host_tools_disabled(&thread_params).expect("host tools disabled");
    assert_elsewhere_mcp_direct_exposure(&thread_params).expect("direct MCP exposure");

    let executable = which_codex_executable().expect("codex installed");
    ensure_codex_mcp_tool_exposure_supported().expect("codex MCP tool exposure support");
    let launch =
        CodexProcessLaunch::from_path(executable).with_env(MCP_BEARER_ENV_VAR, mcp.bearer_token());
    let client = CodexAppServerClient::launch(launch)
        .await
        .expect("launch codex app-server");

    let account = client.account().await.expect("account/read");
    println!("Account category: {:?}", account.account);
    println!("Requires OpenAI auth: {}", account.requires_openai_auth);

    let thread_id = client
        .thread_start_elsewhere(&thread_config)
        .await
        .expect("thread/start");
    println!("Thread id: {thread_id}");

    let tools = client
        .list_mcp_server_tools_named(&thread_id, Some(MCP_SERVER_NAME))
        .await
        .expect("mcpServerStatus/list");
    println!("Discovered MCP tools: {tools:?}");

    for required in [
        "workspace_list",
        "workspace_read",
        "workspace_write",
        "workspace_exec",
    ] {
        if !tools.iter().any(|name| name == required) {
            panic!("missing required MCP tool: {required}");
        }
    }

    println!("Elsewhere MCP server `{MCP_SERVER_NAME}` connected with all workspace tools.");

    client.shutdown().await.expect("shutdown codex");
    mcp.shutdown().await;
}
