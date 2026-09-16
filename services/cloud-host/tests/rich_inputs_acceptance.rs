//! Opt-in real Codex acceptance for Rich Inputs v1. Not run in CI.
//!
//! Real image:
//! 1. Owner uploads a deterministic test image in a hosted conversation.
//! 2. Subscription Codex receives native `localImage` input on the same parent app-server.
//! 3. The Bot correctly describes that image.
//! Never fall back to an OpenAI API key.
//!
//! Real ask_user:
//! 1. Subscription Codex calls `ask_user`.
//! 2. Owner/test harness answers in authenticated Elsewhere UI.
//! 3. The SAME assignment resumes and the final answer uses the selected option.
//!
//! Run with:
//! `ELSEWHERE_RICH_INPUTS_ACCEPTANCE=1 cargo test -p cloud-host --test rich_inputs_acceptance -- --ignored --nocapture`

#[ignore]
#[test]
fn real_codex_image_and_ask_user_acceptance_procedure() {
    if std::env::var("ELSEWHERE_RICH_INPUTS_ACCEPTANCE")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "Set ELSEWHERE_RICH_INPUTS_ACCEPTANCE=1 and follow the module docs to run live Codex acceptance."
        );
        return;
    }
    panic!(
        "Live Codex image + ask_user acceptance is a manual owner procedure; see this file's module docs. \
         Never use an OpenAI API key as a fallback."
    );
}
