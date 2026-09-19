use agent_core::GithubCodingError;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;
use sqlx::PgPool;

use super::check_evidence::{CertifiedCheck, VerifiedCheck};
use super::publish_snapshot::PreparedPublish;

#[derive(Debug, Clone)]
pub struct CodingSessionRow {
    pub owner_id: String,
    pub run_id: String,
    pub request_id: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub full_name: String,
    pub checkout_path: String,
    pub base_branch: String,
    pub opened_base_commit_sha: String,
    pub opened_base_tree_sha: String,
    pub working_branch: String,
    pub local_baseline_commit_sha: String,
    pub reviewed_fingerprint: Option<String>,
    pub approved_fingerprint: Option<String>,
    pub review_completed: bool,
    pub validations: Vec<VerifiedCheck>,
    pub certified_checks: Vec<CertifiedCheck>,
    pub prepared_publish: Option<PreparedPublish>,
    pub checks_passed: Option<bool>,
    pub explicit_no_checks: bool,
    pub publish_phase: Option<String>,
    pub publish_commit_sha: Option<String>,
    pub pr_number: Option<i64>,
    pub pr_url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub struct SessionStore {
    pool: PgPool,
}

impl SessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_open(&self, session: &CodingSessionRow) -> Result<(), GithubCodingError> {
        let validations = serde_json::to_value(&session.validations)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        let certified_checks = serde_json::to_value(&session.certified_checks)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        let prepared_publish = session
            .prepared_publish
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO github_coding_sessions (
                owner_id, run_id, request_id, repo_owner, repo_name, full_name, checkout_path,
                base_branch, opened_base_commit_sha, opened_base_tree_sha, working_branch,
                local_baseline_commit_sha, reviewed_fingerprint, approved_fingerprint,
                review_completed, validations_json, certified_checks_json, prepared_publish_json,
                checks_passed, explicit_no_checks,
                publish_phase, publish_commit_sha, pr_number, pr_url, updated_at
            ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,NOW()
            )
            ON CONFLICT (owner_id, run_id) DO UPDATE SET
                request_id = EXCLUDED.request_id,
                repo_owner = EXCLUDED.repo_owner,
                repo_name = EXCLUDED.repo_name,
                full_name = EXCLUDED.full_name,
                checkout_path = EXCLUDED.checkout_path,
                base_branch = EXCLUDED.base_branch,
                opened_base_commit_sha = EXCLUDED.opened_base_commit_sha,
                opened_base_tree_sha = EXCLUDED.opened_base_tree_sha,
                working_branch = EXCLUDED.working_branch,
                local_baseline_commit_sha = EXCLUDED.local_baseline_commit_sha,
                reviewed_fingerprint = NULL,
                approved_fingerprint = NULL,
                review_completed = FALSE,
                validations_json = '[]'::jsonb,
                certified_checks_json = '[]'::jsonb,
                prepared_publish_json = NULL,
                checks_passed = NULL,
                explicit_no_checks = FALSE,
                publish_phase = NULL,
                publish_commit_sha = NULL,
                pr_number = NULL,
                pr_url = NULL,
                updated_at = NOW()
            "#,
        )
        .bind(&session.owner_id)
        .bind(&session.run_id)
        .bind(&session.request_id)
        .bind(&session.repo_owner)
        .bind(&session.repo_name)
        .bind(&session.full_name)
        .bind(&session.checkout_path)
        .bind(&session.base_branch)
        .bind(&session.opened_base_commit_sha)
        .bind(&session.opened_base_tree_sha)
        .bind(&session.working_branch)
        .bind(&session.local_baseline_commit_sha)
        .bind(&session.reviewed_fingerprint)
        .bind(&session.approved_fingerprint)
        .bind(session.review_completed)
        .bind(validations)
        .bind(certified_checks)
        .bind(prepared_publish)
        .bind(session.checks_passed)
        .bind(session.explicit_no_checks)
        .bind(&session.publish_phase)
        .bind(&session.publish_commit_sha)
        .bind(session.pr_number)
        .bind(&session.pr_url)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn append_certified_check(
        &self,
        owner_id: &str,
        run_id: &str,
        check: &CertifiedCheck,
    ) -> Result<(), GithubCodingError> {
        let value = serde_json::to_value(check)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        sqlx::query(
            r#"
            UPDATE github_coding_sessions SET
                certified_checks_json = certified_checks_json || $3::jsonb,
                updated_at = NOW()
            WHERE owner_id = $1 AND run_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .bind(json!([value]))
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn get(
        &self,
        owner_id: &str,
        run_id: &str,
    ) -> Result<Option<CodingSessionRow>, GithubCodingError> {
        let row = sqlx::query(
            r#"
            SELECT owner_id, run_id, request_id, repo_owner, repo_name, full_name, checkout_path,
                   base_branch, opened_base_commit_sha, opened_base_tree_sha, working_branch,
                   local_baseline_commit_sha, reviewed_fingerprint, approved_fingerprint,
                   review_completed, validations_json, certified_checks_json, prepared_publish_json,
                   checks_passed, explicit_no_checks,
                   publish_phase, publish_commit_sha, pr_number, pr_url, updated_at
            FROM github_coding_sessions
            WHERE owner_id = $1 AND run_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(row.map(map_session_row))
    }

    pub async fn save_review(
        &self,
        owner_id: &str,
        run_id: &str,
        prepared: &PreparedPublish,
        validations: &[VerifiedCheck],
        checks_passed: Option<bool>,
        explicit_no_checks: bool,
    ) -> Result<(), GithubCodingError> {
        let validations_json = serde_json::to_value(validations)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        let prepared_json = serde_json::to_value(prepared)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        sqlx::query(
            r#"
            UPDATE github_coding_sessions SET
                reviewed_fingerprint = $3,
                approved_fingerprint = NULL,
                review_completed = TRUE,
                validations_json = $4,
                prepared_publish_json = $5,
                checks_passed = $6,
                explicit_no_checks = $7,
                updated_at = NOW()
            WHERE owner_id = $1 AND run_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .bind(&prepared.fingerprint)
        .bind(validations_json)
        .bind(prepared_json)
        .bind(checks_passed)
        .bind(explicit_no_checks)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn set_approved_publish(
        &self,
        owner_id: &str,
        run_id: &str,
        prepared: &PreparedPublish,
    ) -> Result<(), GithubCodingError> {
        let prepared_json = serde_json::to_value(prepared)
            .map_err(|e| GithubCodingError::Internal(e.to_string()))?;
        sqlx::query(
            r#"
            UPDATE github_coding_sessions SET
                approved_fingerprint = $3,
                prepared_publish_json = $4,
                updated_at = NOW()
            WHERE owner_id = $1 AND run_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .bind(&prepared.fingerprint)
        .bind(prepared_json)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn update_publish_state(
        &self,
        owner_id: &str,
        run_id: &str,
        phase: &str,
        commit_sha: Option<&str>,
        pr_number: Option<i64>,
        pr_url: Option<&str>,
    ) -> Result<(), GithubCodingError> {
        sqlx::query(
            r#"
            UPDATE github_coding_sessions SET
                publish_phase = $3,
                publish_commit_sha = COALESCE($4, publish_commit_sha),
                pr_number = COALESCE($5, pr_number),
                pr_url = COALESCE($6, pr_url),
                updated_at = NOW()
            WHERE owner_id = $1 AND run_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(run_id)
        .bind(phase)
        .bind(commit_sha)
        .bind(pr_number)
        .bind(pr_url)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn load_run_events(
        &self,
        request_id: &str,
    ) -> Result<Vec<(String, Value)>, GithubCodingError> {
        let rows = sqlx::query(
            r#"
            SELECT event_type, payload_json
            FROM run_events
            WHERE request_id = $1
            ORDER BY id ASC
            "#,
        )
        .bind(request_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get("event_type");
                let payload: Value = row.get("payload_json");
                (event_type, payload)
            })
            .collect())
    }
}

fn map_session_row(row: sqlx::postgres::PgRow) -> CodingSessionRow {
    let validations_json: Value = row.get("validations_json");
    let validations: Vec<VerifiedCheck> = serde_json::from_value(validations_json).unwrap_or_default();
    let certified_json: Value = row.get("certified_checks_json");
    let certified_checks: Vec<CertifiedCheck> =
        serde_json::from_value(certified_json).unwrap_or_default();
    let prepared_json: Option<Value> = row.get("prepared_publish_json");
    let prepared_publish: Option<PreparedPublish> = prepared_json
        .and_then(|v| serde_json::from_value(v).ok());
    CodingSessionRow {
        owner_id: row.get("owner_id"),
        run_id: row.get("run_id"),
        request_id: row.get("request_id"),
        repo_owner: row.get("repo_owner"),
        repo_name: row.get("repo_name"),
        full_name: row.get("full_name"),
        checkout_path: row.get("checkout_path"),
        base_branch: row.get("base_branch"),
        opened_base_commit_sha: row.get("opened_base_commit_sha"),
        opened_base_tree_sha: row.get("opened_base_tree_sha"),
        working_branch: row.get("working_branch"),
        local_baseline_commit_sha: row.get("local_baseline_commit_sha"),
        reviewed_fingerprint: row.get("reviewed_fingerprint"),
        approved_fingerprint: row.get("approved_fingerprint"),
        review_completed: row.get("review_completed"),
        validations,
        certified_checks,
        prepared_publish,
        checks_passed: row.get("checks_passed"),
        explicit_no_checks: row.get("explicit_no_checks"),
        publish_phase: row.get("publish_phase"),
        publish_commit_sha: row.get("publish_commit_sha"),
        pr_number: row.get("pr_number"),
        pr_url: row.get("pr_url"),
        updated_at: row.get("updated_at"),
    }
}

fn map_sql(err: sqlx::Error) -> GithubCodingError {
    GithubCodingError::Internal(err.to_string())
}
