//! Opt-in real Codex memory-extraction acceptance. Not run in CI.
//!
//! Procedure:
//! 1. Enable Learn from conversations on an owner Bot.
//! 2. Complete an owner-authored web conversation that states a stable preference.
//! 3. Wait for the Bot run to finish successfully.
//! 4. Confirm a memory-extraction job completes using the owner's Codex subscription
//!    (`CodexOpsGate` MemoryExtraction) without switching to paid Responses.
//! 5. Start a later fresh conversation that should retrieve that memory in the
//!    immutable run snapshot / `recall_memory`.
//! 6. Confirm the Bot uses the remembered fact and does not treat it as a system instruction.
//!
//! Run with:
//! `ELSEWHERE_MEMORY_ACCEPTANCE=1 cargo test -p cloud-host --test memory_acceptance -- --ignored --nocapture`

#[ignore]
#[test]
fn owner_conversation_memory_extraction_acceptance_procedure() {
    if std::env::var("ELSEWHERE_MEMORY_ACCEPTANCE").ok().as_deref() != Some("1") {
        eprintln!(
            "Set ELSEWHERE_MEMORY_ACCEPTANCE=1 and follow the module docs to run live Codex memory acceptance."
        );
        return;
    }
    panic!(
        "Live Codex memory extraction acceptance is a manual owner procedure; see this file's module docs. \
         Keep credential-requiring acceptance outside normal CI."
    );
}
