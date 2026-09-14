use std::path::PathBuf;

use codex_provider::{
    assert_elsewhere_mcp_direct_exposure, assert_host_tools_disabled,
    build_elsewhere_thread_start_params, ElsewhereThreadConfig,
};

#[test]
fn elsewhere_thread_config_cannot_regress_to_deferred_only_mcp_exposure() {
    let params = build_elsewhere_thread_start_params(&ElsewhereThreadConfig {
        cwd: PathBuf::from("/tmp/elsewhere-empty"),
        mcp_url: "http://127.0.0.1:1234/mcp".into(),
        bearer_env_var: "ELSEWHERE_MCP_TOKEN".into(),
        model: "gpt-5.6-luna".into(),
        base_instructions: None,
        developer_instructions: None,
    })
    .expect("thread params");
    assert_host_tools_disabled(&params).expect("host tools disabled");
    assert_elsewhere_mcp_direct_exposure(&params).expect("direct MCP exposure");
}
