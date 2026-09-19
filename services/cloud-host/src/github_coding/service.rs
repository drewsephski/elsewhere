use std::sync::Arc;

use agent_core::{
    AgentComputer, AgentGithubCoding, ComputerError, GithubCodingError,
    GITHUB_GET_PULL_REQUEST_FEEDBACK_TOOL, GITHUB_OPEN_REPOSITORY_TOOL,
    GITHUB_PUBLISH_PULL_REQUEST_TOOL, GITHUB_RESUME_PULL_REQUEST_TOOL,
    GITHUB_REVIEW_PUBLISH_TOOL, GITHUB_RUN_CHECK_TOOL, GITHUB_UPDATE_PULL_REQUEST_TOOL,
    MAX_EXEC_COMMAND_CHARS,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::connectors::github_client::{GitHubClient, GitHubTreeChange};
use crate::connectors::service::PostgresAgentConnectors;
use crate::github_coding::archive::{
    extract_tarball_files, ArchiveFile, MAX_COMPRESSED_TARBALL_BYTES,
};
use crate::github_coding::check_evidence::{
    reject_forged_check_fields, verify_check_commands_for_review, CertifiedCheck, VerifiedCheck,
};
use crate::github_coding::core::{
    checkout_root, is_elsewhere_managed_branch, path_within_checkout, sanitize_task_slug,
    working_branch,
};
use crate::github_coding::feedback::collect_pull_request_feedback;
use crate::github_coding::publish_snapshot::{
    PreparedPublish, validate_prepared_publish_limits,
};
use crate::github_coding::session_store::{CodingSessionRow, SessionStore};
use crate::github_coding::workspace_git::{
    assert_git_available, collect_publish_changes, init_baseline_repo, reset_checkout_dir,
    GitFileChange,
};
use crate::redact::redact_secrets;
use agent_core::ConnectorError;

const FILES_CHANGED_MSG: &str =
    "Files changed after review. Review the updated changes before publishing.";
const BASE_DRIFT_MSG: &str =
    "The repository changed on GitHub while you were working. Refresh the repository and reapply/review the changes before publishing.";
const BRANCH_COLLISION_MSG: &str =
    "working branch already exists for another Elsewhere change";
const PR_HEAD_DRIFT_MSG: &str =
    "The pull request changed on GitHub while you were working. Refresh the PR before updating it.";
const PR_NOT_ELSEWHERE_MSG: &str =
    "This pull request is not on an Elsewhere-managed branch (elsewhere/*).";
const PR_NOT_OPEN_MSG: &str = "Pull request is not open.";

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
            GITHUB_RUN_CHECK_TOOL => {
                self.run_check(owner_id, run_id, computer, arguments).await
            }
            GITHUB_REVIEW_PUBLISH_TOOL => {
                self.review_publish(owner_id, run_id, request_id, computer, arguments)
                    .await
            }
            GITHUB_PUBLISH_PULL_REQUEST_TOOL => {
                self.publish_pull_request(owner_id, run_id, computer, arguments)
                    .await
            }
            GITHUB_RESUME_PULL_REQUEST_TOOL => {
                self.resume_pull_request(owner_id, run_id, request_id, computer, arguments)
                    .await
            }
            GITHUB_GET_PULL_REQUEST_FEEDBACK_TOOL => {
                self.get_pull_request_feedback(owner_id, run_id, arguments)
                    .await
            }
            GITHUB_UPDATE_PULL_REQUEST_TOOL => {
                self.update_pull_request(owner_id, run_id, computer, arguments)
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
        if tool_name != GITHUB_PUBLISH_PULL_REQUEST_TOOL
            && tool_name != GITHUB_UPDATE_PULL_REQUEST_TOOL
        {
            return Ok(arguments.clone());
        }
        let session = self.require_session(owner_id, run_id).await?;
        if !session.review_completed {
            let hint = if session.session_mode == "revision" {
                "Run github_review_publish before updating the pull request."
            } else {
                "Run github_review_publish before publishing."
            };
            return Err(GithubCodingError::Validation(hint.into()));
        }
        let prepared = self.build_prepared_publish(&session, computer).await?;
        if session.reviewed_fingerprint.as_deref() != Some(prepared.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        let changed_paths = prepared
            .changes
            .iter()
            .map(|c| c.path.clone())
            .collect::<Vec<_>>();
        let verified_checks = validations_to_json(&session.validations);
        let mut merged = arguments.clone();
        if let Some(obj) = merged.as_object_mut() {
            obj.insert("repository".into(), json!(session.full_name));
            obj.insert("branch".into(), json!(session.working_branch));
            obj.insert("baseBranch".into(), json!(session.base_branch));
            obj.insert("changedPaths".into(), json!(changed_paths));
            obj.insert("workspaceFingerprint".into(), json!(prepared.fingerprint));
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
            if tool_name == GITHUB_UPDATE_PULL_REQUEST_TOOL {
                obj.insert(
                    "pullRequestNumber".into(),
                    json!(session.source_pr_number.or(session.pr_number)),
                );
                obj.insert("pullRequestUrl".into(), json!(session.pr_url));
                obj.insert("sessionMode".into(), json!(session.session_mode));
            }
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
        let prepared = self.build_prepared_publish(&session, computer).await?;
        if session.reviewed_fingerprint.as_deref() != Some(prepared.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        self.sessions
            .set_approved_publish(owner_id, run_id, &prepared)
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
            .download_tarball(&token, &owner, &repo, &base_commit_sha)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        if archive_bytes.len() > MAX_COMPRESSED_TARBALL_BYTES {
            return Err(GithubCodingError::Validation(
                "repository archive exceeds maximum download size".into(),
            ));
        }

        let files = extract_tarball_files(&archive_bytes)?;
        let file_count = files.len();
        let checkout_path = checkout_root(&owner, &repo, run_id);
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
            certified_checks: Vec::new(),
            prepared_publish: None,
            checks_passed: None,
            explicit_no_checks: false,
            publish_phase: None,
            publish_commit_sha: None,
            pr_number: None,
            pr_url: None,
            session_mode: "initial".into(),
            source_pr_number: None,
            revision_baseline_commit_sha: None,
            revision_baseline_tree_sha: None,
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

    async fn run_check(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let command = required_str(args, "command")?;
        if command.len() > MAX_EXEC_COMMAND_CHARS {
            return Err(GithubCodingError::Validation(format!(
                "command exceeds {MAX_EXEC_COMMAND_CHARS} characters"
            )));
        }
        let session = self.require_session(owner_id, run_id).await?;
        let prepared_before = self.build_prepared_publish(&session, computer).await?;
        let exec_command = format!(
            "cd {} && {}",
            shell_quote(&session.checkout_path),
            command
        );
        let result = computer
            .exec(&exec_command)
            .await
            .map_err(map_computer_error)?;
        let prepared_after = self.build_prepared_publish(&session, computer).await?;
        if prepared_before.fingerprint != prepared_after.fingerprint {
            return Err(GithubCodingError::Validation(
                "Check modified publishable source files. Re-run the check after your edits settle, or review before changing files."
                    .into(),
            ));
        }
        let exit_code = result.exit_code;
        let ok = result.ok && exit_code == 0;
        let certified = CertifiedCheck {
            command,
            exit_code,
            ok,
            workspace_fingerprint: prepared_before.fingerprint.clone(),
        };
        self.sessions
            .append_certified_check(owner_id, run_id, &certified)
            .await?;
        Ok(json!({
            "ok": ok,
            "command": certified.command,
            "exitCode": exit_code,
            "workspaceFingerprint": certified.workspace_fingerprint,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "phase": "check_certified"
        }))
    }

    async fn review_publish(
        &self,
        owner_id: &str,
        run_id: &str,
        _request_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        reject_forged_check_fields(args)?;
        let session = self.require_session(owner_id, run_id).await?;
        let check_commands = parse_check_commands(args)?;
        let prepared = self.build_prepared_publish(&session, computer).await?;
        let verified = verify_check_commands_for_review(
            &session.certified_checks,
            &check_commands,
            &prepared.fingerprint,
        )?;
        let explicit_no_checks = check_commands.is_empty();
        let checks_passed = if explicit_no_checks {
            None
        } else {
            Some(verified.iter().all(|v| v.ok))
        };

        self.sessions
            .save_review(
                owner_id,
                run_id,
                &prepared,
                &verified,
                checks_passed,
                explicit_no_checks,
            )
            .await?;

        let changed_paths = prepared
            .changes
            .iter()
            .map(|c| c.path.clone())
            .collect::<Vec<_>>();
        let revision = session.session_mode == "revision";
        let phase = if revision {
            "reviewing_revision"
        } else {
            "reviewing_changes"
        };
        let mut body = json!({
            "ok": true,
            "repository": session.full_name,
            "checkoutPath": session.checkout_path,
            "baseBranch": session.base_branch,
            "workingBranch": session.working_branch,
            "changedPaths": changed_paths,
            "changeCount": prepared.changes.len(),
            "verifiedChecks": validations_to_json(&verified),
            "checksPassed": checks_passed,
            "explicitNoChecks": explicit_no_checks,
            "workspaceFingerprint": prepared.fingerprint,
            "phase": phase,
        });
        if let Some(obj) = body.as_object_mut() {
            if revision {
                obj.insert(
                    "readyToUpdatePullRequest".into(),
                    json!(!prepared.changes.is_empty()),
                );
                obj.insert("pullRequestNumber".into(), json!(session.source_pr_number));
            } else {
                obj.insert("readyToPublish".into(), json!(!prepared.changes.is_empty()));
            }
        }
        Ok(body)
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

        let fresh = self.build_prepared_publish(&session, computer).await?;
        if session.reviewed_fingerprint.as_deref() != Some(fresh.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        if session.approved_fingerprint.as_deref() != Some(fresh.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        let prepared = session.prepared_publish.clone().ok_or_else(|| {
            GithubCodingError::Validation(
                "Missing approved publish snapshot. Re-run review and approval.".into(),
            )
        })?;
        if prepared.fingerprint != fresh.fingerprint {
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

        if prepared.changes.is_empty() {
            return Err(GithubCodingError::Validation(
                "No file changes to publish.".into(),
            ));
        }

        let tree_changes = to_github_tree_changes(&prepared.changes);

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
            let adopted = self
                .verify_and_adopt_existing_pull(
                    &session,
                    &prepared,
                    &tree_changes,
                    &token,
                    &existing,
                )
                .await?;
            self.sessions
                .update_publish_state(
                    owner_id,
                    run_id,
                    "pull_request_opened",
                    Some(&adopted.commit_sha),
                    Some(adopted.number),
                    Some(&adopted.url),
                )
                .await?;
            return Ok(json!({
                "ok": true,
                "alreadyExisted": true,
                "repository": session.full_name,
                "branch": session.working_branch,
                "baseBranch": session.base_branch,
                "commitSha": adopted.commit_sha,
                "pullRequest": { "number": adopted.number, "url": adopted.url },
                "phase": "pull_request_opened"
            }));
        }

        if let Some(url) = session.pr_url.as_ref() {
            if let Some(number) = session.pr_number {
                if let Some(commit_sha) = session.publish_commit_sha.as_deref() {
                    return Ok(json!({
                        "ok": true,
                        "alreadyExisted": true,
                        "repository": session.full_name,
                        "branch": session.working_branch,
                        "baseBranch": session.base_branch,
                        "commitSha": commit_sha,
                        "pullRequest": { "number": number, "url": url },
                        "phase": "pull_request_opened"
                    }));
                }
            }
        }
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
            "changedPaths": prepared.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
            "verifiedChecks": validations_to_json(&session.validations),
            "checksPassed": session.checks_passed,
            "phase": "pull_request_opened"
        }))
    }

    async fn build_prepared_publish(
        &self,
        session: &CodingSessionRow,
        computer: &dyn AgentComputer,
    ) -> Result<PreparedPublish, GithubCodingError> {
        let changes = collect_publish_changes(
            computer,
            &session.checkout_path,
            &session.local_baseline_commit_sha,
        )
        .await?;
        let prepared = PreparedPublish::from_changes(
            &session.local_baseline_commit_sha,
            changes,
        );
        validate_prepared_publish_limits(&prepared.changes)?;
        Ok(prepared)
    }

    async fn verify_and_adopt_existing_pull(
        &self,
        session: &CodingSessionRow,
        _prepared: &PreparedPublish,
        tree_changes: &[GitHubTreeChange],
        token: &str,
        existing_pr: &Value,
    ) -> Result<AdoptedPullRequest, GithubCodingError> {
        let number = existing_pr
            .get("number")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i64;
        let url = existing_pr
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let head_sha = self
            .github
            .ref_head_sha(
                token,
                &session.repo_owner,
                &session.repo_name,
                &session.working_branch,
            )
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
            .ok_or_else(|| GithubCodingError::Provider(BRANCH_COLLISION_MSG.into()))?;
        let verified = if session.publish_commit_sha.as_deref() == Some(head_sha.as_str()) {
            true
        } else {
            self.github
                .commit_matches_prepared_changes(
                    token,
                    &session.repo_owner,
                    &session.repo_name,
                    &head_sha,
                    &session.opened_base_commit_sha,
                    tree_changes,
                )
                .await
                .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
        };
        if !verified {
            return Err(GithubCodingError::Provider(BRANCH_COLLISION_MSG.into()));
        }
        Ok(AdoptedPullRequest {
            number,
            url,
            commit_sha: head_sha,
        })
    }

    async fn resume_pull_request(
        &self,
        owner_id: &str,
        run_id: &str,
        request_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let (owner, repo, pr_number) = resolve_pr_target(owner_id, run_id, args, &self.sessions)?;

        self.connectors
            .assert_repo_authorized(owner_id, &owner, &repo)
            .await
            .map_err(Self::map_connector_error)?;

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        let pull = self
            .github
            .get_pull_request(&token, &owner, &repo, pr_number)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        if pull.get("state").and_then(|s| s.as_str()) != Some("open") {
            return Err(GithubCodingError::Validation(PR_NOT_OPEN_MSG.into()));
        }

        let head_ref = pull
            .get("head")
            .and_then(|h| h.get("ref"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| GithubCodingError::Provider("pull request missing head ref".into()))?;
        if !is_elsewhere_managed_branch(head_ref) {
            return Err(GithubCodingError::Validation(PR_NOT_ELSEWHERE_MSG.into()));
        }

        let head_repo = pull
            .get("head")
            .and_then(|h| h.get("repo"))
            .and_then(|r| r.get("full_name"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let expected_full = format!("{}/{}", owner, repo);
        if head_repo != expected_full {
            return Err(GithubCodingError::Validation(
                "Pull request head must be in the authorized repository.".into(),
            ));
        }

        let head_sha = pull
            .get("head")
            .and_then(|h| h.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| GithubCodingError::Provider("pull request missing head sha".into()))?;
        let head_tree_sha = self
            .github
            .get_commit_tree_sha(&token, &owner, &repo, head_sha)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let base_branch = pull
            .get("base")
            .and_then(|b| b.get("ref"))
            .and_then(|s| s.as_str())
            .unwrap_or("main")
            .to_string();

        let archive_bytes = self
            .github
            .download_tarball(&token, &owner, &repo, head_sha)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        if archive_bytes.len() > MAX_COMPRESSED_TARBALL_BYTES {
            return Err(GithubCodingError::Validation(
                "repository archive exceeds maximum download size".into(),
            ));
        }

        let files = extract_tarball_files(&archive_bytes)?;
        let file_count = files.len();
        let checkout_path = checkout_root(&owner, &repo, run_id);
        let pr_url = pull
            .get("html_url")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        computer.ensure_ready().await.map_err(map_computer_error)?;
        assert_git_available(computer).await?;
        reset_checkout_dir(computer, &checkout_path).await?;
        self.materialize_tarball_files(computer, &checkout_path, &files).await?;

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
            opened_base_commit_sha: head_sha.to_string(),
            opened_base_tree_sha: head_tree_sha.clone(),
            working_branch: head_ref.to_string(),
            local_baseline_commit_sha: baseline_commit,
            reviewed_fingerprint: None,
            approved_fingerprint: None,
            review_completed: false,
            validations: Vec::new(),
            certified_checks: Vec::new(),
            prepared_publish: None,
            checks_passed: None,
            explicit_no_checks: false,
            publish_phase: None,
            publish_commit_sha: None,
            pr_number: Some(pr_number as i64),
            pr_url: Some(pr_url),
            session_mode: "revision".into(),
            source_pr_number: Some(pr_number as i64),
            revision_baseline_commit_sha: Some(head_sha.to_string()),
            revision_baseline_tree_sha: Some(head_tree_sha),
            updated_at: chrono::Utc::now(),
        };
        self.sessions.upsert_resume(&row).await?;

        Ok(json!({
            "ok": true,
            "repository": row.full_name,
            "checkoutPath": row.checkout_path,
            "baseBranch": row.base_branch,
            "workingBranch": row.working_branch,
            "pullRequest": { "number": pr_number, "url": row.pr_url },
            "revisionBaselineCommitSha": head_sha,
            "fileCount": file_count,
            "phase": "resuming_pull_request"
        }))
    }

    async fn get_pull_request_feedback(
        &self,
        owner_id: &str,
        run_id: &str,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let (owner, repo, pr_number) = resolve_pr_target(owner_id, run_id, args, &self.sessions)?;

        self.connectors
            .assert_repo_authorized(owner_id, &owner, &repo)
            .await
            .map_err(Self::map_connector_error)?;

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        let pull = self
            .github
            .get_pull_request(&token, &owner, &repo, pr_number)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;

        let head_sha = pull
            .get("head")
            .and_then(|h| h.get("sha"))
            .and_then(|s| s.as_str())
            .unwrap_or("");

        let reviews = self
            .github
            .list_pull_request_reviews(&token, &owner, &repo, pr_number, 50)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        let review_comments = self
            .github
            .list_pull_request_review_comments(&token, &owner, &repo, pr_number, 50)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        let issue_comments = self
            .github
            .list_issue_comments(&token, &owner, &repo, pr_number, 50)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        let combined_status = if head_sha.is_empty() {
            json!({})
        } else {
            self.github
                .get_commit_combined_status(&token, &owner, &repo, head_sha)
                .await
                .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
        };
        let check_runs = if head_sha.is_empty() {
            json!({})
        } else {
            self.github
                .list_commit_check_runs(&token, &owner, &repo, head_sha, 50)
                .await
                .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?
        };

        let feedback = collect_pull_request_feedback(
            &pull,
            &reviews,
            &review_comments,
            &issue_comments,
            &combined_status,
            &check_runs,
        );
        Ok(feedback)
    }

    async fn update_pull_request(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn AgentComputer,
        args: &Value,
    ) -> Result<Value, GithubCodingError> {
        let commit_message = required_str(args, "commitMessage")?;
        let publish_anyway = args
            .get("publishAnyway")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let session = self.require_session(owner_id, run_id).await?;
        if session.session_mode != "revision" {
            return Err(GithubCodingError::Validation(
                "Resume an open Elsewhere pull request before updating it.".into(),
            ));
        }
        if !session.review_completed {
            return Err(GithubCodingError::Validation(
                "Run github_review_publish before updating the pull request.".into(),
            ));
        }

        let fresh = self.build_prepared_publish(&session, computer).await?;
        if session.reviewed_fingerprint.as_deref() != Some(fresh.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        if session.approved_fingerprint.as_deref() != Some(fresh.fingerprint.as_str()) {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }
        let prepared = session.prepared_publish.clone().ok_or_else(|| {
            GithubCodingError::Validation(
                "Missing approved revision snapshot. Re-run review and approval.".into(),
            )
        })?;
        if prepared.fingerprint != fresh.fingerprint {
            return Err(GithubCodingError::Validation(FILES_CHANGED_MSG.into()));
        }

        if session.checks_passed == Some(false) && !publish_anyway {
            return Err(GithubCodingError::Validation(
                "Verified checks did not all pass. Re-run tests or set publishAnyway after owner review."
                    .into(),
            ));
        }

        if prepared.changes.is_empty() {
            return Err(GithubCodingError::Validation(
                "No file changes to push to the pull request.".into(),
            ));
        }

        let baseline_sha = session
            .revision_baseline_commit_sha
            .as_deref()
            .or(Some(session.opened_base_commit_sha.as_str()))
            .ok_or_else(|| GithubCodingError::Validation(PR_HEAD_DRIFT_MSG.into()))?;
        let baseline_tree = session
            .revision_baseline_tree_sha
            .as_deref()
            .or(Some(session.opened_base_tree_sha.as_str()))
            .ok_or_else(|| GithubCodingError::Validation(PR_HEAD_DRIFT_MSG.into()))?;

        self.connectors
            .assert_repo_authorized_fresh(owner_id, &session.repo_owner, &session.repo_name)
            .await
            .map_err(Self::map_connector_error)?;

        let token = self
            .connectors
            .github_access_token_for_owner(owner_id)
            .await
            .map_err(Self::map_connector_error)?;

        let pr_number = session
            .source_pr_number
            .or(session.pr_number)
            .ok_or_else(|| GithubCodingError::Validation("Missing pull request number.".into()))?;

        let pull = self
            .github
            .get_pull_request(&token, &session.repo_owner, &session.repo_name, pr_number as u64)
            .await
            .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
        if pull.get("state").and_then(|s| s.as_str()) != Some("open") {
            return Err(GithubCodingError::Validation(PR_NOT_OPEN_MSG.into()));
        }

        let tree_changes = to_github_tree_changes(&prepared.changes);
        let expected_parent = baseline_sha;

        if let Some(url) = session.pr_url.as_ref() {
            if let Some(commit_sha) = session.publish_commit_sha.as_deref() {
                let head = self
                    .github
                    .ref_head_sha(
                        &token,
                        &session.repo_owner,
                        &session.repo_name,
                        &session.working_branch,
                    )
                    .await
                    .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
                if head.as_deref() == Some(commit_sha) {
                    return Ok(json!({
                        "ok": true,
                        "alreadyUpdated": true,
                        "repository": session.full_name,
                        "branch": session.working_branch,
                        "commitSha": commit_sha,
                        "pullRequest": { "number": pr_number, "url": url },
                        "phase": "pull_request_updated"
                    }));
                }
            }
        }

        self.sessions
            .update_publish_state(owner_id, run_id, "updating_pull_request", None, None, None)
            .await?;

        let commit_sha = if let Some(sha) = session.publish_commit_sha.as_ref() {
            let head = self
                .github
                .ref_head_sha(
                    &token,
                    &session.repo_owner,
                    &session.repo_name,
                    &session.working_branch,
                )
                .await
                .map_err(|e| GithubCodingError::Provider(redact_secrets(&e)))?;
            if head.as_deref() == Some(sha.as_str()) {
                sha.clone()
            } else {
                self.github
                    .update_branch_file_changes(
                        &token,
                        &session.repo_owner,
                        &session.repo_name,
                        &session.working_branch,
                        baseline_sha,
                        baseline_tree,
                        &commit_message,
                        &tree_changes,
                        expected_parent,
                    )
                    .await
                    .map_err(|e| {
                        if e.contains("Refresh the PR") {
                            GithubCodingError::Validation(PR_HEAD_DRIFT_MSG.into())
                        } else {
                            GithubCodingError::Provider(redact_secrets(&e))
                        }
                    })?
            }
        } else {
            self.github
                .update_branch_file_changes(
                    &token,
                    &session.repo_owner,
                    &session.repo_name,
                    &session.working_branch,
                    baseline_sha,
                    baseline_tree,
                    &commit_message,
                    &tree_changes,
                    expected_parent,
                )
                .await
                .map_err(|e| {
                    if e.contains("Refresh the PR") {
                        GithubCodingError::Validation(PR_HEAD_DRIFT_MSG.into())
                    } else {
                        GithubCodingError::Provider(redact_secrets(&e))
                    }
                })?
        };

        self.sessions
            .update_publish_state(
                owner_id,
                run_id,
                "pull_request_updated",
                Some(&commit_sha),
                Some(pr_number),
                session.pr_url.as_deref(),
            )
            .await?;

        Ok(json!({
            "ok": true,
            "repository": session.full_name,
            "branch": session.working_branch,
            "commitSha": commit_sha,
            "pullRequest": {
                "number": pr_number,
                "url": session.pr_url
            },
            "changedPaths": prepared.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
            "verifiedChecks": validations_to_json(&session.validations),
            "checksPassed": session.checks_passed,
            "phase": "pull_request_updated"
        }))
    }

    async fn materialize_tarball_files(
        &self,
        computer: &dyn AgentComputer,
        checkout_path: &str,
        files: &[ArchiveFile],
    ) -> Result<(), GithubCodingError> {
        for file in files {
            let path = path_within_checkout(checkout_path, &file.relative_path)
                .map_err(GithubCodingError::Validation)?;
            let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or(checkout_path);
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
        Ok(())
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
                "Open a repository with github_open_repository or resume a pull request first."
                    .into(),
            ))
    }
}

fn resolve_pr_target(
    owner_id: &str,
    run_id: &str,
    args: &Value,
    sessions: &SessionStore,
) -> Result<(String, String, u64), GithubCodingError> {
    let owner_arg = args.get("owner").and_then(|v| v.as_str());
    let repo_arg = args.get("repo").and_then(|v| v.as_str());
    let number_arg = args
        .get("pullRequestNumber")
        .or_else(|| args.get("prNumber"))
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)));

    if let (Some(owner), Some(repo), Some(number)) = (owner_arg, repo_arg, number_arg) {
        if owner.is_empty() || repo.is_empty() || number == 0 {
            return Err(GithubCodingError::Validation(
                "owner, repo, and pullRequestNumber are required.".into(),
            ));
        }
        return Ok((owner.to_string(), repo.to_string(), number));
    }

    if number_arg.is_some() && (owner_arg.is_none() || repo_arg.is_none()) {
        return Err(GithubCodingError::Validation(
            "Provide owner and repo with pullRequestNumber.".into(),
        ));
    }

    let session = sessions
        .get(owner_id, run_id)
        .await?
        .ok_or_else(|| {
            GithubCodingError::Validation(
                "Provide owner, repo, and pullRequestNumber, or resume the pull request first."
                    .into(),
            )
        })?;
    let number = session
        .source_pr_number
        .or(session.pr_number)
        .ok_or_else(|| {
            GithubCodingError::Validation(
                "No pull request in this session. Provide pullRequestNumber.".into(),
            )
        })?;
    if number <= 0 {
        return Err(GithubCodingError::Validation(
            "Invalid pull request number.".into(),
        ));
    }
    Ok((session.repo_owner, session.repo_name, number as u64))
}

struct AdoptedPullRequest {
    number: i64,
    url: String,
    commit_sha: String,
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
