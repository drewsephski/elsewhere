use std::sync::Arc;

use agent_core::{
    AgentComputer, AgentGithubCoding, ComputerError, GithubCodingError, GITHUB_OPEN_REPOSITORY_TOOL,
    GITHUB_PUBLISH_PULL_REQUEST_TOOL, GITHUB_REVIEW_PUBLISH_TOOL,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::connectors::github_client::{GitHubClient, GitHubTreeChange};
use crate::connectors::service::PostgresAgentConnectors;
use crate::github_coding::archive::{extract_tarball_files, MAX_COMPRESSED_TARBALL_BYTES};
use crate::github_coding::check_evidence::{
    reject_forged_check_fields, verify_check_commands_from_events, VerifiedCheck,
};
use crate::github_coding::core::{checkout_root, path_within_checkout, sanitize_task_slug, working_branch};
use crate::github_coding::session_store::{CodingSessionRow, SessionStore};
use crate::github_coding::workspace_git::{
    assert_git_available, collect_publish_changes, init_baseline_repo, reset_checkout_dir,
    working_tree_fingerprint, GitFileChange,
};
use crate::redact::redact_secrets;
use agent_core::ConnectorError;

const FILES_CHANGED_MSG: &str =
    "Files changed after review. Review the updated changes before publishing.";
const BASE_DRIFT_MSG: &str =
    "The repository changed on GitHub while you were working. Refresh the repository and reapply/review the changes before publishing.";

pub struct PostgresAgentGithubCoding {
    connectors: Arc<PostgresAgentConnectors>,
    github: GitHubClient,
    sessions: SessionStore,
}

impl PostgresAgentGithubCoding {
    pub fn new(
        connectors: Arc<PostgresAgentConnectors>,
        github: GitHubClient,
        pool: PgPool,
    ) -> Arc<Self> {
        Arc::new(Self {
            connectors,
            github,
            sessions: SessionStore::new(pool),
        })
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
        request_id: &str,
        _computer_id: &str,
        computer: &dyn AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        match tool_name {
            GITHUB_OPEN_REPOSITORY_TOOL => {
                self.open_repository(owner_id, run_id, request_id, computer, arguments)
                    .await
            }
            GITHUB_REVIEW_PUBLISH_TOOL => {
                self.review_publish(owner_id, run_id, request_id, computer, arguments)
                    .await
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
        let session = self.require_session(owner_id, run_id).await?;
        if !session.review_completed {
            return Err(GithubCodingError::Validation(
                "Run github_review_publish before publishing.".into(),
            ));
        }
        let fingerprint = working_tree_fingerprint(computer, &session.checkout_path).await?;
        if session.reviewed_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        let changes = collect_publish_changes(computer, &session.checkout_path).await?;
        let changed_paths = changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
        let verified_checks = validations_to_json(&session.validations);
        let mut merged = arguments.clone();
        if let Some(obj) = merged.as_object_mut() {
            obj.insert("repository".into(), json!(session.full_name));
            obj.insert("branch".into(), json!(session.working_branch));
            obj.insert("baseBranch".into(), json!(session.base_branch));
            obj.insert("changedPaths".into(), json!(changed_paths));
            obj.insert("workspaceFingerprint".into(), json!(fingerprint));
            obj.insert("verifiedChecks".into(), verified_checks);
            obj.insert("checksPassed".into(), json!(session.checks_passed));
            obj.insert(
                "explicitNoChecks".into(),
                json!(session.explicit_no_checks),
            );
            obj.insert(
                "publishAnyway".into(),
                json!(arguments
                    .get("publishAnyway")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)),
            );
        }
        Ok(merged)
    }

    async fn confirm_publish_approval(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
    ) -> Result<(), GithubCodingError> {
        let session = self.require_session(owner_id, run_id).await?;
        let fingerprint = working_tree_fingerprint(computer, &session.checkout_path).await?;
        if session.reviewed_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        self.sessions
            .set_approved_fingerprint(owner_id, run_id, &fingerprint)
            .await?;
        Ok(())
    }
}

impl PostgresAgentGithubCoding {
    async fn open_repository(
        &self,
        owner_id: &str,
        run_id: &str,
        request_id: &str,
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

        let base_branch = self
            .github
            .get_repository(&token, &owner, &repo)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
            .get("default_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .to_string();

        let base_commit_sha = self
            .github
            .get_branch_head_sha(&token, &owner, &repo, &base_branch)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        let base_tree_sha = self
            .github
            .get_commit_tree_sha(&token, &owner, &repo, &base_commit_sha)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let archive_bytes = self
            .github
            .download_tarball(&token, &owner, &repo, &base_branch)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        if archive_bytes.len() > MAX_COMPRESSED_TARBALL_BYTES {
            return Err(GithubCodingError::Validation(
                "repository archive exceeds maximum download size".into(),
            ));
        }

        let files = extract_tarball_files(&archive_bytes)?;
        let file_count = files.len();
        let checkout_path = checkout_root(&owner, &repo);
        let branch = working_branch(&slug, run_id);

        computer.ensure_ready().await.map_err(map_computer_error)?;
        assert_git_available(computer).await?;
        reset_checkout_dir(computer, &checkout_path).await?;

        for file in files {
            let path = path_within_checkout(&checkout_path, &file.relative_path)
                .map_err(GithubCodingError::Validation)?;
            let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or(&checkout_path);
            let mkdir = format!("mkdir -p {}", shell_quote(parent));
            exec_ok(computer, &mkdir).await?;
            computer
                .write_file(&path, &file.bytes)
                .await
                .map_err(map_computer_error)?;
            if file.mode & 0o111 != 0 {
                let chmod = format!("chmod +x {}", shell_quote(&path));
                exec_ok(computer, &chmod).await?;
            }
        }

        let baseline_commit = init_baseline_repo(computer, &checkout_path).await?;
        let full_name = format!("{}/{}", owner, repo);
        let row = CodingSessionRow {
            owner_id: owner_id.to_string(),
            run_id: run_id.to_string(),
            request_id: request_id.to_string(),
            repo_owner: owner,
            repo_name: repo,
            full_name,
            checkout_path,
            base_branch,
            opened_base_commit_sha: base_commit_sha,
            opened_base_tree_sha: base_tree_sha,
            working_branch: branch,
            local_baseline_commit_sha: baseline_commit,
            reviewed_fingerprint: None,
            approved_fingerprint: None,
            review_completed: false,
            validations: Vec::new(),
            checks_passed: None,
            explicit_no_checks: false,
            publish_phase: None,
            publish_commit_sha: None,
            pr_number: None,
            pr_url: None,
            updated_at: chrono::Utc::now(),
        };
        self.sessions.upsert_open(&row).await?;

        Ok(json!({
            "ok": true,
            "repository": row.full_name,
            "checkoutPath": row.checkout_path,
            "baseBranch": row.base_branch,
            "workingBranch": row.working_branch,
            "fileCount": file_count,
            "phase": "opening_repository"
        }))
    }

    async fn review_publish(
        &self,
        owner_id: &str,
        run_id: &str,
        request_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        reject_forged_check_fields(args)?;
        let session = self.require_session(owner_id, run_id).await?;
        let check_commands = parse_check_commands(args)?;
        let events = self.sessions.load_run_events(request_id).await?;
        let verified = verify_check_commands_from_events(&events, &check_commands)?;
        let explicit_no_checks = check_commands.is_empty();
        let checks_passed = if explicit_no_checks {
            None
        } else {
            Some(verified.iter().all(|v| v.ok))
        };

        let changes = collect_publish_changes(computer, &session.checkout_path).await?;
        let fingerprint = working_tree_fingerprint(computer, &session.checkout_path).await?;
        self.sessions
            .save_review(
                owner_id,
                run_id,
                &fingerprint,
                &verified,
                checks_passed,
                explicit_no_checks,
            )
            .await?;

        let changed_paths = changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "repository": session.full_name,
            "checkoutPath": session.checkout_path,
            "baseBranch": session.base_branch,
            "workingBranch": session.working_branch,
            "changedPaths": changed_paths,
            "changeCount": changes.len(),
            "verifiedChecks": validations_to_json(&verified),
            "checksPassed": checks_passed,
            "explicitNoChecks": explicit_no_checks,
            "workspaceFingerprint": fingerprint,
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

        let session = self.require_session(owner_id, run_id).await?;
        if !session.review_completed {
            return Err(GithubCodingError::Validation(
                "Run github_review_publish before publishing.".into(),
            ));
        }

        let fingerprint = working_tree_fingerprint(computer, &session.checkout_path).await?;
        if session.reviewed_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        if session.approved_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }

        if session.checks_passed == Some(false) && !publish_anyway {
            return Err(GithubCodingError::Validation(
                "Verified checks did not all pass. Re-run tests or set publishAnyway after owner review."
                    .into(),
            ));
        }

        self.connectors
            .assert_repo_authorized_fresh(owner_id, &session.repo_owner, &session.repo_name)
            .await
            .map_err(Self::map_connector_error)?;

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        let current_base = self
            .github
            .get_branch_head_sha(
                &token,
                &session.repo_owner,
                &session.repo_name,
                &session.base_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        if current_base != session.opened_base_commit_sha {
            return Err(GithubCodingError::Validation(BASE_DRIFT_MSG.into()));
        }

        let changes = collect_publish_changes(computer, &session.checkout_path).await?;
        if changes.is_empty() {
            return Err(GithubCodingError::Validation(
                "No file changes to publish.".into(),
            ));
        }

        if let Some(existing) = self
            .github
            .find_open_pull_for_head(
                &token,
                &session.repo_owner,
                &session.repo_name,
                &session.working_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
        {
            let number = existing.get("number").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            let url = existing
                .get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            self.sessions
                .update_publish_state(owner_id, run_id, "pull_request_opened", None, Some(number), Some(&url))
                .await?;
            return Ok(json!({
                "ok": true,
                "alreadyExisted": true,
                "repository": session.full_name,
                "branch": session.working_branch,
                "baseBranch": session.base_branch,
                "pullRequest": { "number": number, "url": url },
                "phase": "pull_request_opened"
            }));
        }

        if let Some(url) = session.pr_url.as_ref() {
            if session.pr_number.is_some() {
                return Ok(json!({
                    "ok": true,
                    "alreadyExisted": true,
                    "repository": session.full_name,
                    "branch": session.working_branch,
                    "baseBranch": session.base_branch,
                    "pullRequest": {
                        "number": session.pr_number,
                        "url": url
                    },
                    "phase": "pull_request_opened"
                }));
            }
        }

        let tree_changes = to_github_tree_changes(&changes);
        let commit_message = format!("{title}\n\n{body}");
        let expected_commit = session.publish_commit_sha.as_deref();

        self.sessions
            .update_publish_state(owner_id, run_id, "publishing_commit", None, None, None)
            .await?;

        let commit_sha = if let Some(sha) = session.publish_commit_sha.as_ref() {
            sha.clone()
        } else {
            let sha = self
                .github
                .publish_file_changes(
                    &token,
                    &session.repo_owner,
                    &session.repo_name,
                    &session.opened_base_commit_sha,
                    &session.opened_base_tree_sha,
                    &session.working_branch,
                    &commit_message,
                    &tree_changes,
                    expected_commit,
                )
                .await
                .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
            self.sessions
                .update_publish_state(owner_id, run_id, "branch_published", Some(&sha), None, None)
                .await?;
            sha
        };

        self.sessions
            .update_publish_state(owner_id, run_id, "opening_pull_request", Some(&commit_sha), None, None)
            .await?;

        let pull = self
            .github
            .create_pull_request(
                &token,
                &session.repo_owner,
                &session.repo_name,
                &title,
                &body,
                &session.working_branch,
                &session.base_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let number = pull.get("number").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
        let url = pull
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        self.sessions
            .update_publish_state(
                owner_id,
                run_id,
                "pull_request_opened",
                Some(&commit_sha),
                Some(number),
                Some(&url),
            )
            .await?;

        Ok(json!({
            "ok": true,
            "repository": session.full_name,
            "branch": session.working_branch,
            "baseBranch": session.base_branch,
            "commitSha": commit_sha,
            "pullRequest": { "number": number, "url": url },
            "changedPaths": changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
            "verifiedChecks": validations_to_json(&session.validations),
            "checksPassed": session.checks_passed,
            "phase": "pull_request_opened"
        }))
    }

    async fn require_session(
        &self,
        owner_id: &str,
        run_id: &str,
    ) -> Result<CodingSessionRow, GithubCodingError> {
        self.sessions
            .get(owner_id, run_id)
            .await?
            .ok_or(GithubCodingError::Validation(
                "Open a repository with github_open_repository first.".into(),
            ))
    }
}

fn parse_check_commands(args: &Value) -> Result<Vec<String>, GithubCodingError> {
    let Some(raw) = args.get("checkCommands") else {
        return Ok(Vec::new());
    };
    let array = raw.as_array().ok_or_else(|| {
        GithubCodingError::Validation("checkCommands must be an array of strings".into())
    })?;
    let mut out = Vec::new();
    for item in array {
        let cmd = item.as_str().ok_or_else(|| {
            GithubCodingError::Validation("checkCommands entries must be strings".into())
        })?;
        if cmd.trim().is_empty() {
            return Err(GithubCodingError::Validation(
                "checkCommands entries must be non-empty".into(),
            ));
        }
        out.push(cmd.to_string());
    }
    Ok(out)
}

fn validations_to_json(validations: &[VerifiedCheck]) -> Value {
    json!(
        validations
            .iter()
            .map(|v| {
                json!({
                    "command": v.command,
                    "exitCode": v.exit_code,
                    "ok": v.ok
                })
            })
            .collect::<Vec<_>>()
    )
}

fn to_github_tree_changes(changes: &[GitFileChange]) -> Vec<GitHubTreeChange> {
    changes
        .iter()
        .map(|c| GitHubTreeChange {
            path: c.path.clone(),
            mode: c.mode.clone(),
            deleted: c.deleted,
            content: c.bytes.clone(),
        })
        .collect()
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

async fn exec_ok(computer: &dyn AgentComputer, command: &str) -> Result<(), GithubCodingError> {
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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
