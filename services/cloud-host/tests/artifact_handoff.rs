use agent_core::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use async_trait::async_trait;
use cloud_host::{artifact_handoff, result_finalization, run_lifecycle, work};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

struct MemoryComputer {
    files: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl AgentComputer for MemoryComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        Ok(Vec::new())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| ComputerError::ExecutionFailed(format!("missing {path}")))
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ComputerError> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), data.to_vec());
        Ok(())
    }

    async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
        Ok(ExecResult {
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        })
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn completed_target_without_results_finalization_does_not_resume(pool: PgPool) {
    let chief_id = cloud_host::db::resources::insert_computer_placeholder(&pool, "alice", "C1")
        .await
        .unwrap()
        .id;
    let researcher_id =
        cloud_host::db::resources::insert_computer_placeholder(&pool, "alice", "C2")
            .await
            .unwrap()
            .id;
    let chief = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Chief",
        "",
        "gpt-5.6-luna",
        Some(&chief_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let researcher = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Researcher",
        "",
        "gpt-5.6-luna",
        Some(&researcher_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();

    let source = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &chief.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    let created = cloud_host::delegation::create_delegation(
        &pool,
        &agent_core::CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: chief.id.clone(),
            source_run_id: source.run_id.clone(),
            source_conversation_id: source.conversation_id.clone(),
            source_request_id: source.request_id.clone(),
            tool_invocation_id: "ordering".into(),
        },
        &researcher.id,
        "Do work",
        None,
        "resume_source",
    )
    .await
    .unwrap();

    sqlx::query("UPDATE agent_runs SET status = 'completed', results_status = 'pending' WHERE id = $1")
        .bind(&created.target_run_id)
        .execute(&pool)
        .await
        .unwrap();

    cloud_host::delegation::sync_target_run_terminal(
        &pool,
        &sqlx::query_scalar::<_, String>("SELECT request_id FROM agent_runs WHERE id = $1")
            .bind(&created.target_run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed",
        None,
    )
    .await
    .unwrap();

    let resume: Option<String> = sqlx::query_scalar(
        "SELECT source_resume_run_id FROM bot_delegations WHERE id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(resume.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn same_computer_skips_copy_with_results_path(pool: PgPool) {
    let computer_id =
        cloud_host::db::resources::insert_computer_placeholder(&pool, "alice", "Shared")
            .await
            .unwrap()
            .id;
    let chief = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Chief",
        "",
        "gpt-5.6-luna",
        Some(&computer_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let researcher = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Researcher",
        "",
        "gpt-5.6-luna",
        Some(&computer_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();

    let source = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &chief.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    let created = cloud_host::delegation::create_delegation(
        &pool,
        &agent_core::CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: chief.id,
            source_run_id: source.run_id,
            source_conversation_id: source.conversation_id,
            source_request_id: source.request_id,
            tool_invocation_id: "shared".into(),
        },
        &researcher.id,
        "Files",
        None,
        "resume_source",
    )
    .await
    .unwrap();

    let result_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO work_results (id, run_id, name, kind, content) VALUES ($1,$2,'a.md','file',$3)",
    )
    .bind(result_id)
    .bind(&created.target_run_id)
    .bind(b"hello".as_slice())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&created.target_run_id)
        .execute(&pool)
        .await
        .unwrap();

    result_finalization::finalize_collection(&pool, &created.target_run_id, "complete", None)
        .await
        .unwrap();
    artifact_handoff::plan_transfers_for_delegation(&pool, &created.delegation_id)
        .await
        .unwrap();

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "skipped");
    let path: String = sqlx::query_scalar(
        "SELECT destination_path FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(path.contains(&created.target_run_id));
}
