use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_core::{
    AgentComputer, AgentGithubCoding, ComputerError, GithubCodingError, GITHUB_OPEN_REPOSITORY_TOOL,
    GITHUB_PUBLISH_PULL_REQUEST_TOOL, GITHUB_REVIEW_PUBLISH_TOOL,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::connectors::service::PostgresAgentConnectors;
use crate::github_coding::archive::extract_tarball_text_files;
use agent_core::ConnectorError;
use crate::connectors::GitHubClient;
use crate::github_coding::core::{
    checkout_root, diff_against_baseline, path_within_checkout, sanitize_task_slug, summarize_changes,
    working_branch,
};
use crate::redact::redact_secrets;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationRecord {
    command: String,
    exit_code: i32,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GithubCodingSession {
    owner: String,
    repo: String,
    full_name: String,
    checkout_path: String,
    base_branch: String,
    working_branch: String,
    baseline: HashMap<String, Vec<u8>>,
    validations: Vec<ValidationRecord>,
    checks_passed: Option<bool>,
    last_pull_request: Option<PullRequestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PullRequestRecord {
    number: u64,
    url: String,
    branch: String,
    base_branch: String,
    commit_sha: String,
}

pub struct PostgresAgentGithubCoding {
    connectors: Arc<PostgresAgentConnectors>,
    github: GitHubClient,
    sessions: Mutex<HashMap<(String, String), GithubCodingSession>>,
}

impl PostgresAgentGithubCoding {
    pub fn new(connectors: Arc<PostgresAgentConnectors>, github: GitHubClient) -> Arc<Self> {
        Arc::new(Self {
            connectors,
            github,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn session_key(owner_id: &str, run_id: &str) -> (String, String) {
        (owner_id.to_string(), run_id.to_string())
    }

    fn map_connector_error(err: ConnectorError) -> GithubCodingError {
        match err {
            ConnectorError::NotConnected => GithubCodingError::NotConnected,
            ConnectorError::ReconnectRequired => GithubCodingError::ReconnectRequired,
            ConnectorError::NotFound => GithubCodingError::NotFound,
            ConnectorError::Validation(m) => GithubCodingError::Validation(m),
            ConnectorError::Provider(m) => GithubCodingError::Provider(redact_secrets(&m)),
            ConnectorError::Internal(m) => GithubCodingError::Internal(m),
        }
    }
}

#[async_trait]
impl AgentGithubCoding for PostgresAgentGithubCoding {
    async fn dispatch_tool(
        &self,
        owner_id: &str,
        run_id: &str,
        _computer_id: &str,
        computer: &dyn AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        match tool_name {
            GITHUB_OPEN_REPOSITORY_TOOL => {
                self.open_repository(owner_id, run_id, computer, arguments).await
            }
            GITHUB_REVIEW_PUBLISH_TOOL => {
                self.review_publish(owner_id, run_id, computer, arguments).await
            }
            GITHUB_PUBLISH_PULL_REQUEST_TOOL => {
                self.publish_pull_request(owner_id, run_id, computer, arguments)
                    .await
            }
            other => Err(GithubCodingError::Validation(format!("unknown tool: {other}"))),
        }
    }

    async fn approval_arguments(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        if tool_name != GITHUB_PUBLISH_PULL_REQUEST_TOOL {
            return Ok(arguments.clone());
        }
        let session = self
            .load_session(owner_id, run_id)?
            .ok_or(GithubCodingError::Validation(
                "Open a repository before publishing.".into(),
            ))?;
        let current = read_baseline_from_computer_async(computer, &session).await?;
        let changes = diff_against_baseline(&session.baseline, &current);
        let mut merged = arguments.clone();
        if let Some(obj) = merged.as_object_mut() {
            obj.insert("repository".into(), json!(session.full_name));
            obj.insert("branch".into(), json!(session.working_branch));
            obj.insert("baseBranch".into(), json!(session.base_branch));
            obj.insert("changedPaths".into(), json!(summarize_changes(&changes)));
            obj.insert("checksPassed".into(), json!(session.checks_passed));
        }
        Ok(merged)
    }
}

impl PostgresAgentGithubCoding {
    async fn open_repository(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let owner = required_str(args, "owner")?;
        let repo = required_str(args, "repo")?;
        let slug = args
            .get("taskSlug")
            .and_then(|v| v.as_str())
            .map(sanitize_task_slug)
            .transpose()
            .map_err(GithubCodingError::Validation)?
            .unwrap_or_else(|| {
                sanitize_task_slug(&format!("{}-{}", owner, repo)).unwrap_or_else(|_| "task".into())
            });

        self.connectors
            .assert_repo_authorized(owner_id, &owner, &repo)
            .await
            .map_err(Self::map_connector_error)?;

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        let repository = self
            .github
            .get_repository(&token, &owner, &repo)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        let base_branch = repository
            .get("default_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .to_string();

        let archive_bytes = self
            .github
            .download_tarball(&token, &owner, &repo, &base_branch)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let files = extract_tarball_text_files(&archive_bytes)?;
        let checkout_path = checkout_root(&owner, &repo);
        let branch = working_branch(&slug);

        computer
            .ensure_ready()
            .await
            .map_err(map_computer_error)?;

        let mkdir = format!("mkdir -p {}", shell_quote(&checkout_path));
        exec_ok_async(computer, &mkdir).await?;

        let mut baseline = HashMap::new();
        for (relative, data) in files {
            let path = path_within_checkout(&checkout_path, &relative)
                .map_err(GithubCodingError::Validation)?;
            computer
                .write_file(&path, &data)
                .await
                .map_err(map_computer_error)?;
            baseline.insert(relative, data);
        }

        let full_name = format!("{}/{}", owner, repo);
        let session = GithubCodingSession {
            owner,
            repo,
            full_name,
            checkout_path,
            base_branch,
            working_branch: branch,
            baseline,
            validations: Vec::new(),
            checks_passed: None,
            last_pull_request: None,
        };
        let file_count = session.baseline.len();
        let response = json!({
            "ok": true,
            "repository": session.full_name,
            "checkoutPath": session.checkout_path,
            "baseBranch": session.base_branch,
            "workingBranch": session.working_branch,
            "fileCount": file_count,
            "phase": "opening_repository"
        });
        self.sessions
            .lock()
            .expect("github coding sessions")
            .insert(Self::session_key(owner_id, run_id), session);
        Ok(response)
    }

    async fn review_publish(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let session = self
            .load_session(owner_id, run_id)?
            .ok_or(GithubCodingError::Validation(
                "Open a repository with github_open_repository before reviewing changes."
                    .into(),
            ))?;

        if let Some(checks) = args.get("checks").and_then(|v| v.as_array()) {
            let mut validations = Vec::new();
            for item in checks {
                let command = item
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let exit_code = item
                    .get("exitCode")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1) as i32;
                let ok = item.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                validations.push(ValidationRecord {
                    command,
                    exit_code,
                    ok,
                });
            }
            self.update_session(owner_id, run_id, |s| {
                s.validations = validations;
                s.checks_passed = Some(s.validations.iter().all(|v| v.ok));
            })?;
        }

        let session = self.load_session(owner_id, run_id)?.expect("session");
        let current = read_baseline_from_computer_async(computer, &session).await?;
        let changes = diff_against_baseline(&session.baseline, &current);
        let changed_paths = summarize_changes(&changes);

        Ok(json!({
            "ok": true,
            "repository": session.full_name,
            "checkoutPath": session.checkout_path,
            "baseBranch": session.base_branch,
            "workingBranch": session.working_branch,
            "changedPaths": changed_paths,
            "changeCount": changes.len(),
            "validations": session.validations,
            "checksPassed": session.checks_passed,
            "readyToPublish": !changes.is_empty(),
            "phase": "reviewing_changes"
        }))
    }

    async fn publish_pull_request(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let title = required_str(args, "title")?;
        let body = required_str(args, "body")?;
        let publish_anyway = args
            .get("publishAnyway")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let session = self
            .load_session(owner_id, run_id)?
            .ok_or(GithubCodingError::Validation(
                "Open a repository before publishing.".into(),
            ))?;

        if session.checks_passed == Some(false) && !publish_anyway {
            return Err(GithubCodingError::Validation(
                "Recorded checks did not all pass. Re-run tests or set publishAnyway after owner review."
                    .into(),
            ));
        }

        let current = read_baseline_from_computer_async(computer, &session).await?;
        let changes = diff_against_baseline(&session.baseline, &current);
        if changes.is_empty() {
            return Err(GithubCodingError::Validation(
                "No file changes to publish.".into(),
            ));
        }

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        if let Some(existing) = self
            .github
            .find_open_pull_for_head(
                &token,
                &session.owner,
                &session.repo,
                &session.working_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
        {
            let number = existing.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
            let url = existing
                .get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let record = PullRequestRecord {
                number,
                url,
                branch: session.working_branch.clone(),
                base_branch: session.base_branch.clone(),
                commit_sha: String::new(),
            };
            self.update_session(owner_id, run_id, |s| {
                s.last_pull_request = Some(record.clone());
            })?;
            return Ok(json!({
                "ok": true,
                "alreadyExisted": true,
                "repository": session.full_name,
                "branch": session.working_branch,
                "baseBranch": session.base_branch,
                "pullRequest": {
                    "number": record.number,
                    "url": record.url
                },
                "phase": "pull_request_opened"
            }));
        }

        let base_sha = self
            .github
            .get_branch_head_sha(&token, &session.owner, &session.repo, &session.base_branch)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let mut publish_files: Vec<(String, String)> = Vec::new();
        for change in &changes {
            if let Some(current) = &change.current {
                if let Ok(text) = String::from_utf8(current.clone()) {
                    publish_files.push((change.path.clone(), text));
                }
            }
        }
        if publish_files.is_empty() {
            return Err(GithubCodingError::Validation(
                "Only text file changes can be published in this slice.".into(),
            ));
        }

        let commit_message = format!("{title}\n\n{body}");
        let commit_sha = self
            .github
            .publish_tree_commit(
                &token,
                &session.owner,
                &session.repo,
                &base_sha,
                &session.working_branch,
                &commit_message,
                &publish_files
                    .iter()
                    .map(|(p, c)| (p.as_str(), c.as_str()))
                    .collect::<Vec<_>>(),
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let pull = self
            .github
            .create_pull_request(
                &token,
                &session.owner,
                &session.repo,
                &title,
                &body,
                &session.working_branch,
                &session.base_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let number = pull.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
        let url = pull
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let record = PullRequestRecord {
            number,
            url,
            branch: session.working_branch.clone(),
            base_branch: session.base_branch.clone(),
            commit_sha,
        };
        self.update_session(owner_id, run_id, |s| {
            s.last_pull_request = Some(record.clone());
        })?;

        Ok(json!({
            "ok": true,
            "repository": session.full_name,
            "branch": session.working_branch,
            "baseBranch": session.base_branch,
            "commitSha": record.commit_sha,
            "pullRequest": {
                "number": record.number,
                "url": record.url
            },
            "changedPaths": summarize_changes(&changes),
            "validations": session.validations,
            "checksPassed": session.checks_passed,
            "phase": "pull_request_opened"
        }))
    }

    fn load_session(
        &self,
        owner_id: &str,
        run_id: &str,
    ) -> Result<Option<GithubCodingSession>, GithubCodingError> {
        Ok(self
            .sessions
            .lock()
            .expect("github coding sessions")
            .get(&Self::session_key(owner_id, run_id))
            .cloned())
    }

    fn update_session(
        &self,
        owner_id: &str,
        run_id: &str,
        update: impl FnOnce(&mut GithubCodingSession),
    ) -> Result<(), GithubCodingError> {
        let mut guard = self.sessions.lock().expect("github coding sessions");
        let entry = guard
            .get_mut(&Self::session_key(owner_id, run_id))
            .ok_or_else(|| GithubCodingError::Validation("coding session not found".into()))?;
        update(entry);
        Ok(())
    }
}

fn required_str(args: &Value, key: &str) -> Result<String, GithubCodingError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| GithubCodingError::Validation(format!("missing or empty `{key}`")))
}

fn map_computer_error(err: ComputerError) -> GithubCodingError {
    GithubCodingError::Provider(err.to_string())
}

async fn exec_ok_async(
    computer: &dyn AgentComputer,
    command: &str,
) -> Result<(), GithubCodingError> {
    let result = computer.exec(command).await.map_err(map_computer_error)?;
    if !result.ok {
        return Err(GithubCodingError::Provider(format!(
            "command failed ({}): {}",
            result.exit_code,
            redact_secrets(&result.stderr)
        )));
    }
    Ok(())
}

async fn read_baseline_from_computer_async(
    computer: &dyn AgentComputer,
    session: &GithubCodingSession,
) -> Result<HashMap<String, Vec<u8>>, GithubCodingError> {
    let mut current = HashMap::new();
    for relative in session.baseline.keys() {
        let path = path_within_checkout(&session.checkout_path, relative)
            .map_err(GithubCodingError::Validation)?;
        match computer.read_file(&path).await {
            Ok(bytes) => current.insert(relative.clone(), bytes),
            Err(ComputerError::ExecutionFailed(_)) => continue,
            Err(err) => return Err(map_computer_error(err)),
        };
    }
    Ok(current)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
