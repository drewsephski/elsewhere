use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use codex_provider::{
    probe_codex_subscription_availability_on_client, CodexAppServerClient, CodexProcessLaunch,
    CodexSubscriptionAvailability,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::warn;

pub const CODEX_BUSY_REASON: &str = "codex_busy";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexOperationKind {
    Probe,
    Run,
    Login,
    Archive,
    GroupRoute,
}

impl CodexOperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Run => "run",
            Self::Login => "login",
            Self::Archive => "archive",
            Self::GroupRoute => "group_route",
        }
    }
}

#[derive(Clone)]
pub struct CodexOpsGate {
    semaphore: Arc<Semaphore>,
    active_children: Arc<AtomicUsize>,
}

pub struct CodexOpsPermit {
    permit: OwnedSemaphorePermit,
    kind: CodexOperationKind,
    wait_started: Instant,
    acquired_at: Instant,
    gate: CodexOpsGate,
}

impl CodexOpsPermit {
    pub fn kind(&self) -> CodexOperationKind {
        self.kind
    }

    pub fn log_child_started(&self) {
        tracing::info!(
            target: "elsewhere_codex_ops",
            operation = self.kind.as_str(),
            waiting_for_codex_permit_ms = self.wait_started.elapsed().as_millis() as u64,
            permit_acquired = true,
            child_started = true,
            active_codex_child_count = self.gate.active_children.load(Ordering::SeqCst),
            "codex child process started"
        );
    }

    pub fn log_child_finished(&self) {
        let duration_ms = self.acquired_at.elapsed().as_millis() as u64;
        tracing::info!(
            target: "elsewhere_codex_ops",
            operation = self.kind.as_str(),
            child_finished = true,
            operation_duration_ms = duration_ms,
            active_codex_child_count = self.gate.active_children.load(Ordering::SeqCst),
            "codex child operation finished"
        );
    }
}

impl Drop for CodexOpsPermit {
    fn drop(&mut self) {
        self.log_child_finished();
        let previous = self.gate.active_children.fetch_sub(1, Ordering::SeqCst);
        if previous > 1 {
            warn!(
                target: "elsewhere_codex_ops",
                active_codex_child_count = previous - 1,
                "codex active child count invariant violated"
            );
        }
    }
}

impl CodexOpsGate {
    pub fn new(semaphore: Arc<Semaphore>) -> Self {
        Self {
            semaphore,
            active_children: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn from_permits(permits: usize) -> Self {
        Self::new(Arc::new(Semaphore::new(permits)))
    }

    pub fn semaphore(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    pub fn active_children(&self) -> usize {
        self.active_children.load(Ordering::SeqCst)
    }

    pub async fn acquire(&self, kind: CodexOperationKind) -> Result<CodexOpsPermit, ()> {
        let wait_started = Instant::now();
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ())?;
        let acquired_at = Instant::now();
        let active = self.active_children.fetch_add(1, Ordering::SeqCst) + 1;
        if active > 1 {
            warn!(
                target: "elsewhere_codex_ops",
                active_codex_child_count = active,
                operation = kind.as_str(),
                "codex active child count exceeded one"
            );
        }
        tracing::info!(
            target: "elsewhere_codex_ops",
            operation = kind.as_str(),
            waiting_for_codex_permit_ms = wait_started.elapsed().as_millis() as u64,
            permit_acquired = true,
            active_codex_child_count = active,
            "codex operation permit acquired"
        );
        Ok(CodexOpsPermit {
            permit,
            kind,
            wait_started,
            acquired_at,
            gate: self.clone(),
        })
    }

    pub fn try_acquire(&self, kind: CodexOperationKind) -> Result<CodexOpsPermit, ()> {
        let wait_started = Instant::now();
        let permit = self.semaphore.clone().try_acquire_owned().map_err(|_| ())?;
        let acquired_at = Instant::now();
        let active = self.active_children.fetch_add(1, Ordering::SeqCst) + 1;
        if active > 1 {
            warn!(
                target: "elsewhere_codex_ops",
                active_codex_child_count = active,
                operation = kind.as_str(),
                "codex active child count exceeded one"
            );
        }
        tracing::info!(
            target: "elsewhere_codex_ops",
            operation = kind.as_str(),
            waiting_for_codex_permit_ms = wait_started.elapsed().as_millis() as u64,
            permit_acquired = true,
            active_codex_child_count = active,
            "codex operation permit acquired (try)"
        );
        Ok(CodexOpsPermit {
            permit,
            kind,
            wait_started,
            acquired_at,
            gate: self.clone(),
        })
    }
}

pub async fn probe_subscription_with_profile(
    executable: Option<std::path::PathBuf>,
    profile: Option<std::path::PathBuf>,
    permit: &CodexOpsPermit,
) -> CodexSubscriptionAvailability {
    use codex_provider::{which_codex_executable, CodexProviderError};

    let executable = match executable.or_else(|| which_codex_executable().ok()) {
        Some(path) => path,
        None => return CodexSubscriptionAvailability::NotInstalled,
    };

    let mut launch = CodexProcessLaunch::from_path(executable).subscription_child();
    if let Some(profile) = profile {
        launch = launch.with_profile(&profile);
    }

    const PROBE_STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
    let client =
        match tokio::time::timeout(PROBE_STARTUP_TIMEOUT, CodexAppServerClient::launch(launch))
            .await
        {
            Ok(Ok(client)) => client,
            Ok(Err(CodexProviderError::CodexNotInstalled)) => {
                return CodexSubscriptionAvailability::NotInstalled;
            }
            Ok(Err(err)) => return CodexSubscriptionAvailability::Unavailable(err.to_string()),
            Err(_) => {
                return CodexSubscriptionAvailability::Unavailable(
                    "Codex app-server startup timed out during availability probe".into(),
                );
            }
        };
    permit.log_child_started();

    let availability = probe_codex_subscription_availability_on_client(&client).await;
    if let Err(err) = client.shutdown().await {
        tracing::debug!(error = %err, "codex probe client shutdown");
    }
    availability
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_slot_gate() -> CodexOpsGate {
        CodexOpsGate::from_permits(1)
    }

    #[test]
    fn allows_only_one_active_operation() {
        let gate = single_slot_gate();
        let run = gate
            .try_acquire(CodexOperationKind::Run)
            .expect("run permit");
        assert_eq!(gate.active_children(), 1);
        assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
        assert!(gate.try_acquire(CodexOperationKind::Login).is_err());
        drop(run);
        assert_eq!(gate.active_children(), 0);
        assert!(gate.try_acquire(CodexOperationKind::Probe).is_ok());
    }

    #[test]
    fn provider_status_and_run_do_not_overlap() {
        let gate = single_slot_gate();
        let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
        assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
        drop(run);
        let probe = gate.try_acquire(CodexOperationKind::Probe).expect("probe");
        assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
        drop(probe);
    }

    #[test]
    fn auto_probe_and_run_share_one_slot() {
        let gate = single_slot_gate();
        let probe = gate.try_acquire(CodexOperationKind::Probe).expect("probe");
        assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
        drop(probe);
        let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
        assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
        drop(run);
    }

    #[test]
    fn device_login_blocks_provider_status_probe() {
        let gate = single_slot_gate();
        let login = gate.try_acquire(CodexOperationKind::Login).expect("login");
        assert!(gate.try_acquire(CodexOperationKind::Probe).is_err());
        drop(login);
    }

    #[test]
    fn device_login_blocks_run_admission() {
        let gate = single_slot_gate();
        let login = gate.try_acquire(CodexOperationKind::Login).expect("login");
        assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
        drop(login);
    }

    #[test]
    fn group_route_waits_behind_run_permit() {
        let gate = single_slot_gate();
        let run = gate.try_acquire(CodexOperationKind::Run).expect("run");
        assert!(gate.try_acquire(CodexOperationKind::GroupRoute).is_err());
        drop(run);
        assert!(gate.try_acquire(CodexOperationKind::GroupRoute).is_ok());
    }

    #[test]
    fn archive_shares_slot_with_run() {
        let gate = single_slot_gate();
        let archive = gate
            .try_acquire(CodexOperationKind::Archive)
            .expect("archive");
        assert!(gate.try_acquire(CodexOperationKind::Run).is_err());
        drop(archive);
        assert!(gate.try_acquire(CodexOperationKind::Run).is_ok());
    }
}
