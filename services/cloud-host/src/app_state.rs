use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_provider::CodexAppServerClient;
use sqlx::PgPool;
use tokio::sync::{Mutex, Semaphore};

use agent_core::{AgentComputer, ResponsesModel};

use crate::approval::ApprovalService;
use crate::auth::JwtVerifier;
use crate::channels::slack::SlackClient;
use crate::codex_ops::CodexOpsPermit;
use crate::computer_registry::ComputerRegistry;
use crate::config::Config;
use crate::connectors::{ConnectorSecretBox, GitHubClient};
use crate::events::registry::RunRegistry;
use crate::human_intervention::HumanInterventionService;
use crate::permission_policies::PermissionPolicyService;
use crate::provider_status_cache::ProviderStatusCache;
use crate::user_questions::UserQuestionService;

#[cfg(any(test, feature = "test-utils"))]
#[derive(Clone)]
pub struct TestRunOverrides {
    pub computer: Arc<dyn AgentComputer>,
    pub model: Arc<dyn ResponsesModel>,
}

#[cfg(any(test, feature = "test-utils"))]
pub type TestGroupRouteDecider = Arc<
    dyn Fn(
            &crate::group_router::RouteDecisionInput,
        ) -> Result<crate::group_router::ValidatedRouteDecision, String>
        + Send
        + Sync,
>;

pub struct PendingCodexLogin {
    pub owner_id: String,
    pub login_id: String,
    pub auth_url: String,
    pub user_code: String,
    pub expires_at: std::time::Instant,
    pub client: Arc<CodexAppServerClient>,
    pub codex_permit: CodexOpsPermit,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub registry: Arc<RunRegistry>,
    pub run_semaphore: Arc<Semaphore>,
    pub jwt_verifier: Option<Arc<JwtVerifier>>,
    pub codex_login_client: Arc<Mutex<Option<PendingCodexLogin>>>,
    pub approvals: ApprovalService,
    pub permission_policies: PermissionPolicyService,
    pub human_interventions: HumanInterventionService,
    pub user_questions: UserQuestionService,
    pub draining: Arc<std::sync::atomic::AtomicBool>,
    pub run_tasks: Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
    pub group_route_tasks: Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
    pub runner_heartbeat: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    pub computer_registry: ComputerRegistry,
    /// One Codex app-server child at a time (probe, login, runs) on this host.
    pub codex_ops: crate::codex_ops::CodexOpsGate,
    pub dispatcher_alive: Arc<std::sync::atomic::AtomicBool>,
    pub provider_status_cache: ProviderStatusCache,
    /// Limits concurrent background group routing tasks (not Codex permits).
    pub group_route_semaphore: Arc<Semaphore>,
    pub connector_secret_box: Option<Arc<ConnectorSecretBox>>,
    pub github_client: GitHubClient,
    pub slack_client: SlackClient,
    #[cfg(any(test, feature = "test-utils"))]
    pub test_group_route_decider: Arc<std::sync::Mutex<Option<TestGroupRouteDecider>>>,
    #[cfg(any(test, feature = "test-utils"))]
    test_run_overrides: TestRunOverrideRegistry,
    #[cfg(any(test, feature = "test-utils"))]
    pub test_memory_extractor: Arc<
        std::sync::Mutex<
            Option<
                Arc<
                    dyn Fn(
                            &crate::memory::extraction::ExtractionRequest,
                        )
                            -> Result<crate::memory::extraction::ExtractionModelResponse, String>
                        + Send
                        + Sync,
                >,
            >,
        >,
    >,
}

#[cfg(any(test, feature = "test-utils"))]
#[derive(Clone, Default)]
struct TestRunOverrideRegistry {
    by_request_id: Arc<std::sync::Mutex<HashMap<String, TestRunOverrides>>>,
    default: Arc<std::sync::Mutex<Option<TestRunOverrides>>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl TestRunOverrideRegistry {
    fn register(&self, request_id: &str, overrides: TestRunOverrides) {
        self.by_request_id
            .lock()
            .expect("test run overrides lock")
            .insert(request_id.to_string(), overrides);
    }

    fn set_default(&self, overrides: Option<TestRunOverrides>) {
        *self
            .default
            .lock()
            .expect("test run overrides default lock") = overrides;
    }

    fn clear_request(&self, request_id: &str) {
        self.by_request_id
            .lock()
            .expect("test run overrides lock")
            .remove(request_id);
    }

    fn take_for_request(&self, request_id: &str) -> Option<TestRunOverrides> {
        let mut by_id = self.by_request_id.lock().expect("test run overrides lock");
        if let Some(overrides) = by_id.remove(request_id) {
            return Some(overrides);
        }
        self.default
            .lock()
            .expect("test run overrides default lock")
            .clone()
    }
}

impl AppState {
    pub fn connector_secret_box(&self) -> Option<Arc<ConnectorSecretBox>> {
        self.connector_secret_box.clone()
    }
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let permits = config.max_concurrent_runs;
        let jwt_verifier = match (
            config.jwt_jwks_url.as_ref(),
            config.jwt_issuer.as_ref(),
            config.jwt_audience.as_ref(),
        ) {
            (Some(jwks), Some(iss), Some(aud)) => {
                Some(Arc::new(JwtVerifier::new(crate::auth::JwtVerifierConfig {
                    jwks_url: jwks.clone(),
                    issuer: iss.clone(),
                    audience: aud.clone(),
                })))
            }
            _ => None,
        };
        let approvals = ApprovalService {
            pool: pool.clone(),
            registry: Arc::new(crate::approval::ApprovalWaitRegistry::default()),
            timeout: Duration::from_secs(config.tool_approval_timeout_secs),
        };
        let permission_policies = PermissionPolicyService::new(pool.clone());
        let human_interventions = HumanInterventionService {
            pool: pool.clone(),
            registry: Arc::new(crate::approval::ApprovalWaitRegistry::default()),
        };
        let user_questions = UserQuestionService {
            pool: pool.clone(),
            registry: Arc::new(crate::approval::ApprovalWaitRegistry::default()),
        };
        let connector_secret_box = config
            .connector_secret_key
            .as_deref()
            .and_then(|key| ConnectorSecretBox::from_base64_key(key).ok())
            .map(Arc::new);
        let github_client = GitHubClient::production();
        let slack_api_base = config.slack_api_base.clone();
        let slack_oauth_base = slack_api_base.trim_end_matches("/api").to_string();
        let slack_client = SlackClient::with_bases(slack_api_base, slack_oauth_base);

        Self {
            pool,
            config: Arc::new(config),
            registry: Arc::new(RunRegistry::default()),
            run_semaphore: Arc::new(Semaphore::new(permits)),
            jwt_verifier,
            codex_login_client: Arc::new(Mutex::new(None)),
            approvals,
            permission_policies,
            human_interventions,
            user_questions,
            draining: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            run_tasks: Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new())),
            group_route_tasks: Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new())),
            runner_heartbeat: Arc::new(std::sync::Mutex::new(None)),
            computer_registry: ComputerRegistry::default(),
            codex_ops: crate::codex_ops::CodexOpsGate::from_permits(1),
            dispatcher_alive: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            provider_status_cache: ProviderStatusCache::default(),
            group_route_semaphore: Arc::new(Semaphore::new(2)),
            connector_secret_box,
            github_client,
            slack_client,
            #[cfg(any(test, feature = "test-utils"))]
            test_group_route_decider: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(any(test, feature = "test-utils"))]
            test_run_overrides: TestRunOverrideRegistry::default(),
            #[cfg(any(test, feature = "test-utils"))]
            test_memory_extractor: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Binds injected computer/model dependencies to a specific idempotency key before `POST /v1/runs`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn register_test_run_overrides(&self, request_id: &str, overrides: TestRunOverrides) {
        self.test_run_overrides.register(request_id, overrides);
    }

    /// Applies the same injected dependencies to any run whose request id was not registered explicitly.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn set_test_run_overrides_default(&self, overrides: Option<TestRunOverrides>) {
        self.test_run_overrides.set_default(overrides);
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn clear_test_run_overrides(&self, request_id: &str) {
        self.test_run_overrides.clear_request(request_id);
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn take_test_run_overrides(&self, request_id: &str) -> Option<TestRunOverrides> {
        self.test_run_overrides.take_for_request(request_id)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn set_test_group_route_decider(&self, decider: Option<TestGroupRouteDecider>) {
        *self
            .test_group_route_decider
            .lock()
            .expect("test group route decider lock") = decider;
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn set_test_memory_extractor(
        &self,
        extractor: Option<
            Arc<
                dyn Fn(
                        &crate::memory::extraction::ExtractionRequest,
                    )
                        -> Result<crate::memory::extraction::ExtractionModelResponse, String>
                    + Send
                    + Sync,
            >,
        >,
    ) {
        *self
            .test_memory_extractor
            .lock()
            .expect("test memory extractor lock") = extractor;
    }
}
