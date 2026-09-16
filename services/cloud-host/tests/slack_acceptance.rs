//! Opt-in real Slack acceptance. Not run in CI.
//!
//! Procedure:
//! 1. Create a Slack app with bot scopes `chat:write`, `app_mentions:read`, `im:history`.
//! 2. Set Events API request URL to `$ELSEWHERE_WEB_ORIGIN/api/channels/slack/events`.
//! 3. Subscribe to `message.im` and `app_mention`.
//! 4. Export `SLACK_CLIENT_ID`, `SLACK_CLIENT_SECRET`, `SLACK_SIGNING_SECRET`,
//!    `SLACK_OAUTH_REDIRECT_URI`, and host secrets as in `.env.example`.
//! 5. Connect Slack from `/app/channels` while signed in as the Elsewhere owner.
//! 6. DM the Slack app: a durable Elsewhere conversation/run should appear, the Bot
//!    should execute with the owner's ChatGPT/Codex path, and the reply should land
//!    in the same Slack DM.
//! 7. Send a message that requires approval. Slack should get one
//!    "{Bot} needs your approval in Elsewhere." notice with a Work link.
//!    Approve in the authenticated web UI. The same run resumes and the final
//!    Slack response is posted afterward.
//!
//! Run with:
//! `ELSEWHERE_SLACK_ACCEPTANCE=1 cargo test -p cloud-host --test slack_acceptance -- --ignored --nocapture`

#[ignore]
#[test]
fn slack_owner_dm_and_approval_notice_acceptance_procedure() {
    if std::env::var("ELSEWHERE_SLACK_ACCEPTANCE").ok().as_deref() != Some("1") {
        eprintln!(
            "Set ELSEWHERE_SLACK_ACCEPTANCE=1 and follow the module docs to run live Slack acceptance."
        );
        return;
    }
    panic!(
        "Live Slack acceptance is a manual owner procedure; see this file's module docs. \
         Do not automate against production Slack from CI."
    );
}
