//! Bounded autonomous browser recovery hints and human-escalation classification.
//!
//! Policy lives here; the LLM still chooses tools. We enrich tool results/errors and enforce
//! snapshot-after-handback for browser mutations.

use serde_json::{json, Value};
use std::sync::Mutex;

use crate::tool_catalog::is_browser_mutation_tool;
use crate::tools::ToolError;

/// Max autonomous recovery cycles for the same recoverable failure class before escalation is recommended.
pub const MAX_RECOVERY_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverableFailureKind {
    StaleRef,
    Transient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanBlockerHint {
    pub reason: &'static str,
    pub guidance: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryHint {
    pub kind: RecoverableFailureKind,
    pub next_steps: &'static str,
    pub attempts_used: u8,
    pub attempts_remaining: u8,
    pub escalation_recommended: bool,
}

#[derive(Debug, Default)]
struct BrowserRecoveryState {
    recovery_attempts: u8,
    last_recoverable_kind: Option<RecoverableFailureKind>,
    snapshot_required_before_mutation: bool,
}

/// Per-run browser recovery / handback state (shared across MCP and Responses dispatch).
pub struct BrowserRecoverySession {
    state: Mutex<BrowserRecoveryState>,
}

impl BrowserRecoverySession {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(BrowserRecoveryState::default()),
        }
    }

    pub fn mark_owner_handback(&self) {
        let mut state = self.state.lock().expect("browser recovery lock");
        state.snapshot_required_before_mutation = true;
        state.recovery_attempts = 0;
        state.last_recoverable_kind = None;
    }

    pub fn preflight_browser_tool(&self, tool_name: &str) -> Result<(), ToolError> {
        if tool_name != "browser_snapshot" && is_browser_mutation_tool(tool_name) {
            let state = self.state.lock().expect("browser recovery lock");
            if state.snapshot_required_before_mutation {
                return Err(ToolError::Denied(
                    "Call browser_snapshot first after owner handback, then reassess the page before mutating."
                        .into(),
                ));
            }
        }
        Ok(())
    }

    pub fn on_browser_success(&self, tool_name: &str, result: Value) -> Value {
        let mut state = self.state.lock().expect("browser recovery lock");
        if tool_name == "browser_snapshot" {
            state.snapshot_required_before_mutation = false;
            state.recovery_attempts = 0;
            state.last_recoverable_kind = None;
            return enrich_snapshot_success(result);
        }
        if is_browser_mutation_tool(tool_name) {
            state.recovery_attempts = 0;
            state.last_recoverable_kind = None;
        }
        result
    }

    pub fn on_browser_failure(&self, tool_name: &str, err: ToolError) -> ToolError {
        if !is_browser_mutation_tool(tool_name) {
            return err;
        }
        let message = err.message();
        let kind = classify_recoverable_failure(&message);
        if kind.is_none() {
            return err;
        }
        let kind = kind.unwrap();
        let mut state = self.state.lock().expect("browser recovery lock");
        if state.last_recoverable_kind == Some(kind) {
            state.recovery_attempts = state.recovery_attempts.saturating_add(1);
        } else {
            state.last_recoverable_kind = Some(kind);
            state.recovery_attempts = 1;
        }
        let attempts_used = state.recovery_attempts;
        let attempts_remaining = MAX_RECOVERY_ATTEMPTS.saturating_sub(attempts_used);
        let escalation_recommended = attempts_used >= MAX_RECOVERY_ATTEMPTS;
        let hint = RecoveryHint {
            kind,
            next_steps: recovery_next_steps(kind),
            attempts_used,
            attempts_remaining,
            escalation_recommended,
        };
        enrich_failure_with_recovery(err, &hint)
    }
}

pub fn browser_recovery_policy_instructions() -> &'static str {
    "Browser recovery: On stale refs, timeouts, or unexpected page changes, call browser_snapshot and retry with a bounded plan (snapshot → reassess → alternate safe action). Do not loop blindly; after a few failed recovery attempts on the same issue, call browser_request_human. Escalate immediately (do not retry) for CAPTCHA, login/credentials, 2FA/OTP, passkeys/biometrics, or owner consent/destructive confirmations. After browser_request_human resolves, always browser_snapshot first, reassess from scratch, and do not assume the owner completed the step you expected."
}

/// Classify snapshot text for human-only blockers. Uses coarse keywords only (no page dumps).
pub fn detect_human_blocker(snapshot_text: &str) -> Option<HumanBlockerHint> {
    let lower = snapshot_text.to_ascii_lowercase();

    if contains_any(
        &lower,
        &[
            "captcha",
            "recaptcha",
            "hcaptcha",
            "verify you are human",
            "not a robot",
            "are you human",
        ],
    ) {
        return Some(HumanBlockerHint {
            reason: "captcha",
            guidance: "Complete the CAPTCHA or anti-bot check in the browser, then return control.",
        });
    }

    if contains_any(
        &lower,
        &[
            "two-factor",
            "two factor",
            "2fa",
            "one-time code",
            "one time code",
            "verification code",
            "authenticator app",
            "enter the code",
            "otp",
        ],
    ) {
        return Some(HumanBlockerHint {
            reason: "two_factor",
            guidance:
                "Complete two-factor or OTP verification in the browser, then return control.",
        });
    }

    if contains_any(
        &lower,
        &[
            "passkey",
            "security key",
            "webauthn",
            "use your fingerprint",
            "touch id",
            "face id",
        ],
    ) {
        return Some(HumanBlockerHint {
            reason: "passkey",
            guidance:
                "Complete passkey or biometric verification in the browser, then return control.",
        });
    }

    if (contains_any(&lower, &["sign in", "log in", "login", "sign-in"]))
        && contains_any(
            &lower,
            &[
                "password",
                "email",
                "username",
                "forgot password",
                "create account",
            ],
        )
    {
        return Some(HumanBlockerHint {
            reason: "login",
            guidance: "Sign in with your account in the browser, then return control.",
        });
    }

    if contains_any(
        &lower,
        &[
            "enter your password",
            "api key",
            "secret key",
            "credentials",
        ],
    ) {
        return Some(HumanBlockerHint {
            reason: "credentials",
            guidance: "Enter required credentials in the browser yourself, then return control.",
        });
    }

    if contains_any(
        &lower,
        &[
            "allow notifications",
            "grant permission",
            "enable location",
            "cookie consent",
            "accept all cookies",
            "privacy choices",
            "terms of service",
            "i agree",
        ],
    ) && contains_any(
        &lower,
        &["permission", "consent", "cookies", "terms", "agree"],
    ) {
        return Some(HumanBlockerHint {
            reason: "consent",
            guidance: "Make the required permission or consent choice in the browser, then return control.",
        });
    }

    None
}

pub fn classify_recoverable_failure(error_message: &str) -> Option<RecoverableFailureKind> {
    let lower = error_message.to_ascii_lowercase();
    if lower.contains("unknown ref") || lower.contains("call browser_snapshot first") {
        return Some(RecoverableFailureKind::StaleRef);
    }
    if lower.contains("timeout") || lower.contains("timed out") || lower.contains("navigation") {
        return Some(RecoverableFailureKind::Transient);
    }
    None
}

pub fn plan_recovery_after_failure(
    attempts_already_used: u8,
    kind: RecoverableFailureKind,
) -> RecoveryHint {
    let attempts_used = attempts_already_used.saturating_add(1);
    let attempts_remaining = MAX_RECOVERY_ATTEMPTS.saturating_sub(attempts_used);
    RecoveryHint {
        kind,
        next_steps: recovery_next_steps(kind),
        attempts_used,
        attempts_remaining,
        escalation_recommended: attempts_used >= MAX_RECOVERY_ATTEMPTS,
    }
}

fn recovery_next_steps(kind: RecoverableFailureKind) -> &'static str {
    match kind {
        RecoverableFailureKind::StaleRef => {
            "Call browser_snapshot, pick a fresh ref or alternate safe control, then retry the action."
        }
        RecoverableFailureKind::Transient => {
            "Call browser_snapshot to inspect the page; if the task URL is wrong, browser_navigate to a safe URL, then retry."
        }
    }
}

fn enrich_snapshot_success(mut result: Value) -> Value {
    let snapshot_blob = extract_snapshot_text(&result);
    if let Some(hint) = detect_human_blocker(&snapshot_blob) {
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "humanBlocker".into(),
                json!({
                    "reason": hint.reason,
                    "guidance": hint.guidance,
                    "escalate": true,
                    "tool": "browser_request_human"
                }),
            );
        }
    }
    result
}

fn extract_snapshot_text(result: &Value) -> String {
    let mut parts = Vec::new();
    for key in ["summary", "text", "url", "title"] {
        if let Some(s) = result.get(key).and_then(|v| v.as_str()) {
            parts.push(s.to_string());
        }
    }
    if let Some(elements) = result.get("elements").and_then(|v| v.as_array()) {
        for el in elements {
            if let Some(s) = el.as_str() {
                parts.push(s.to_string());
            }
        }
    }
    parts.join("\n")
}

fn enrich_failure_with_recovery(err: ToolError, hint: &RecoveryHint) -> ToolError {
    let message = format_recovery_message(err.message(), hint);
    match err {
        ToolError::ComputerNotReady(_) => {
            ToolError::ComputerNotReady(crate::computer::ComputerError::ExecutionFailed(message))
        }
        ToolError::MalformedArguments(_) => ToolError::MalformedArguments(message),
        ToolError::Denied(_) => ToolError::Denied(message),
        ToolError::Cancelled => ToolError::Cancelled,
    }
}

fn format_recovery_message(base: String, hint: &RecoveryHint) -> String {
    format!(
        "{base} [recovery: {next_steps} attempts={used}/{max}{escalate}]",
        next_steps = hint.next_steps,
        used = hint.attempts_used,
        max = MAX_RECOVERY_ATTEMPTS,
        escalate = if hint.escalation_recommended {
            "; call browser_request_human if still blocked"
        } else {
            ""
        }
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captcha_snapshot_triggers_human_blocker_hint() {
        let hint = detect_human_blocker("Please complete the reCAPTCHA to continue").unwrap();
        assert_eq!(hint.reason, "captcha");
    }

    #[test]
    fn two_factor_snapshot_triggers_escalation_reason() {
        let hint = detect_human_blocker("Enter the verification code from your authenticator app")
            .unwrap();
        assert_eq!(hint.reason, "two_factor");
    }

    #[test]
    fn recoverable_stale_ref_not_immediate_escalation() {
        let kind =
            classify_recoverable_failure("unknown ref e3; call browser_snapshot first").unwrap();
        let plan = plan_recovery_after_failure(0, kind);
        assert!(!plan.escalation_recommended);
        assert_eq!(plan.attempts_remaining, 2);
    }

    #[test]
    fn repeated_recovery_failure_recommends_escalation() {
        let kind = RecoverableFailureKind::StaleRef;
        let plan = plan_recovery_after_failure(MAX_RECOVERY_ATTEMPTS - 1, kind);
        assert!(plan.escalation_recommended);
        assert_eq!(plan.attempts_remaining, 0);
    }

    #[test]
    fn handback_requires_snapshot_before_mutation() {
        let session = BrowserRecoverySession::new();
        session.mark_owner_handback();
        let err = session.preflight_browser_tool("browser_click").unwrap_err();
        assert!(matches!(err, ToolError::Denied(_)));
        assert!(session.preflight_browser_tool("browser_snapshot").is_ok());
    }

    #[test]
    fn snapshot_after_handback_clears_mutation_gate() {
        let session = BrowserRecoverySession::new();
        session.mark_owner_handback();
        let _ =
            session.on_browser_success("browser_snapshot", json!({"ok": true, "summary": "ok"}));
        assert!(session.preflight_browser_tool("browser_click").is_ok());
    }

    #[test]
    fn snapshot_enrichment_flags_captcha_without_calling_human_tool() {
        let session = BrowserRecoverySession::new();
        let out = session.on_browser_success(
            "browser_snapshot",
            json!({"ok": true, "summary": "Solve captcha to proceed"}),
        );
        assert_eq!(
            out.get("humanBlocker")
                .and_then(|v| v.get("reason"))
                .and_then(|v| v.as_str()),
            Some("captcha")
        );
    }

    #[test]
    fn bounded_recovery_on_mutation_failure() {
        let session = BrowserRecoverySession::new();
        let err = session.on_browser_failure(
            "browser_click",
            ToolError::ComputerNotReady(crate::computer::ComputerError::ExecutionFailed(
                "unknown ref e9; call browser_snapshot first".into(),
            )),
        );
        let msg = err.message();
        assert!(msg.contains("browser_snapshot"));
        assert!(msg.contains("attempts=1/3"));
        assert!(!msg.contains("browser_request_human"));
    }

    #[test]
    fn third_recovery_failure_recommends_human_escalation() {
        let session = BrowserRecoverySession::new();
        let base = ToolError::ComputerNotReady(crate::computer::ComputerError::ExecutionFailed(
            "unknown ref e9; call browser_snapshot first".into(),
        ));
        let _ = session.on_browser_failure("browser_click", base.clone());
        let _ = session.on_browser_failure("browser_click", base.clone());
        let err = session.on_browser_failure("browser_click", base);
        assert!(err.message().contains("browser_request_human"));
    }
}
