use codex_provider::CodexAccountKind;
use codex_provider::{
    codex_version, probe_codex, which_codex_executable, CodexAppServerClient, CodexProcessLaunch,
};

#[tokio::main]
async fn main() {
    let executable = which_codex_executable().expect("codex installed");
    let version = codex_version(&executable).expect("codex version");
    println!("Codex installed: {executable:?}");
    println!("Version: {version}");

    let cli_probe = probe_codex().ok();
    if let Some(probe) = &cli_probe {
        println!("CLI probe auth: {:?}", probe.auth);
    }

    let client = CodexAppServerClient::launch(CodexProcessLaunch::from_path(executable))
        .await
        .expect("launch app-server");

    let account = client.account().await.expect("account/read");
    print_account(&account);

    if let Ok(limits) = client.rate_limits().await {
        print_rate_limits(&limits.raw);
    }

    let _ = client.shutdown().await;
}

fn print_account(account: &codex_provider::CodexAccountState) {
    match &account.account {
        CodexAccountKind::ChatGpt { email, plan_type } => {
            println!("Auth: ChatGPT");
            if let Some(email) = email {
                println!("Account email: {email}");
            }
            println!("Plan: {plan_type}");
        }
        CodexAccountKind::ApiKey => println!("Auth: API key"),
        CodexAccountKind::NotLoggedIn => println!("Auth: not logged in"),
        CodexAccountKind::Other(kind) => println!("Auth: other ({kind})"),
    }
    println!("Requires OpenAI auth: {}", account.requires_openai_auth);
}

fn print_rate_limits(raw: &serde_json::Value) {
    let plan = raw
        .get("planType")
        .or_else(|| raw.pointer("/codexRateLimits/planType"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    println!("Rate limits planType: {plan}");
    if let Some(allowed) = raw.get("allowed").and_then(|v| v.as_bool()) {
        println!("Rate limits allowed: {allowed}");
    }
}
