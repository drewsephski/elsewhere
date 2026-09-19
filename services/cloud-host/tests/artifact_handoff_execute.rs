use agent_core::{AgentComputer, FakeAgentComputer};
use cloud_host::{artifact_handoff, result_finalization, work, AppState};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

mod support;

struct CrossComputerSetup {
    chief_computer_id: String,
    researcher_computer_id: String,
    delegation_id: String,
    target_run_id: String,
    destination_path: String,
}

async fn cross_computer_file_delegation(pool: &PgPool, invoke: &str) -> CrossComputerSetup {
    let chief_id = cloud_host::db::resources::insert_computer_placeholder(pool, "alice", "C1")
        .await
        .unwrap()
        .id;
    let researcher_id = cloud_host::db::resources::insert_computer_placeholder(pool, "alice", "C2")
        .await
        .unwrap()
        .id;
    let chief = cloud_host::db::resources::insert_bot(
        pool,
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
        pool,
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
        pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &chief.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    let created = cloud_host::delegation::create_delegation(
        pool,
        &agent_core::CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: chief.id,
            source_run_id: source.run_id,
            source_conversation_id: source.conversation_id,
            source_request_id: source.request_id,
            tool_invocation_id: invoke.into(),
        },
        &researcher.id,
        "Files",
        None,
        "resume_source",
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO work_results (id, run_id, name, kind, content) VALUES ($1,$2,'notes.md','file',$3)",
    )
    .bind(Uuid::new_v4())
    .bind(&created.target_run_id)
    .bind(b"cited findings".as_slice())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&created.target_run_id)
        .execute(pool)
        .await
        .unwrap();
    result_finalization::finalize_collection(pool, &created.target_run_id, "complete", None)
        .await
        .unwrap();
    artifact_handoff::plan_transfers_for_delegation(pool, &created.delegation_id)
        .await
        .unwrap();
    let destination_path: String = sqlx::query_scalar(
        "SELECT destination_path FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(pool)
    .await
    .unwrap();

    CrossComputerSetup {
        chief_computer_id: chief_id,
        researcher_computer_id: researcher_id,
        delegation_id: created.delegation_id,
        target_run_id: created.target_run_id,
        destination_path,
    }
}

async fn complete_target_and_resume(pool: &PgPool, target_run_id: &str, delegation_id: &str) {
    let target_request: String =
        sqlx::query_scalar("SELECT request_id FROM agent_runs WHERE id = $1")
            .bind(target_run_id)
            .fetch_one(pool)
            .await
            .unwrap();
    cloud_host::delegation::sync_target_run_terminal(pool, &target_request, "completed", None)
        .await
        .unwrap();
    cloud_host::run_lifecycle::try_admit_delegation_return(pool, delegation_id)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn sprite_to_sprite_copy_uses_generic_agent_computer(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "sprite-sprite").await;
    let dest = Arc::new(FakeAgentComputer::new());
    let state = AppState::new(pool.clone(), support::test_config());
    state
        .computer_registry
        .register_test_computer("alice", &setup.chief_computer_id, dest.clone());

    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(
        dest.file_contents(&setup.destination_path).as_deref(),
        Some(b"cited findings".as_slice())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sprite_to_local_mac_copy_uses_generic_agent_computer(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "sprite-mac").await;
    sqlx::query("UPDATE sandboxes SET provider = 'local_mac' WHERE id = $1")
        .bind(&setup.chief_computer_id)
        .execute(&pool)
        .await
        .unwrap();
    let dest = Arc::new(FakeAgentComputer::new());
    let state = AppState::new(pool.clone(), support::test_config());
    state
        .computer_registry
        .register_test_computer("alice", &setup.chief_computer_id, dest.clone());

    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(
        dest.file_contents(&setup.destination_path).as_deref(),
        Some(b"cited findings".as_slice())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn local_mac_to_sprite_copy_uses_generic_agent_computer(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "mac-sprite").await;
    sqlx::query("UPDATE sandboxes SET provider = 'local_mac' WHERE id = $1")
        .bind(&setup.researcher_computer_id)
        .execute(&pool)
        .await
        .unwrap();
    let dest = Arc::new(FakeAgentComputer::new());
    let state = AppState::new(pool.clone(), support::test_config());
    state
        .computer_registry
        .register_test_computer("alice", &setup.chief_computer_id, dest.clone());

    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(
        dest.file_contents(&setup.destination_path).as_deref(),
        Some(b"cited findings".as_slice())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn destination_unavailable_fails_instead_of_staying_transferring(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "dest-gone").await;
    sqlx::query("UPDATE sandboxes SET state = 'archived' WHERE id = $1")
        .bind(&setup.chief_computer_id)
        .execute(&pool)
        .await
        .unwrap();
    let state = AppState::new(pool.clone(), support::test_config());
    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    let (status, code): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_code FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(code.as_deref(), Some("destination_unavailable"));
}

#[sqlx::test(migrations = "./migrations")]
async fn abandoned_transfer_is_reclaimed_then_completed(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "reclaim").await;
    sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'transferring', started_at = NOW() - INTERVAL '1 hour'
        WHERE delegation_id = $1
        "#,
    )
    .bind(&setup.delegation_id)
    .execute(&pool)
    .await
    .unwrap();

    artifact_handoff::reclaim_abandoned_transfers_older_than(&pool, Duration::from_secs(60))
        .await
        .unwrap();
    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "pending");

    let dest = Arc::new(FakeAgentComputer::new());
    let state = AppState::new(pool.clone(), support::test_config());
    state
        .computer_registry
        .register_test_computer("alice", &setup.chief_computer_id, dest.clone());
    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar(
        "SELECT status FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
}

#[sqlx::test(migrations = "./migrations")]
async fn second_abandoned_transfer_becomes_failed(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "lease-exhausted").await;
    sqlx::query(
        r#"
        UPDATE delegation_artifact_transfers
        SET status = 'transferring',
            error_code = 'reclaimed',
            started_at = NOW() - INTERVAL '1 hour'
        WHERE delegation_id = $1
        "#,
    )
    .bind(&setup.delegation_id)
    .execute(&pool)
    .await
    .unwrap();

    artifact_handoff::reclaim_abandoned_transfers_older_than(&pool, Duration::from_secs(60))
        .await
        .unwrap();
    let (status, code): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_code FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(code.as_deref(), Some("transfer_interrupted"));
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_transfer_still_resumes_source_with_truthful_artifact_status(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "fail-resume").await;
    sqlx::query("UPDATE sandboxes SET state = 'archived' WHERE id = $1")
        .bind(&setup.chief_computer_id)
        .execute(&pool)
        .await
        .unwrap();
    let state = AppState::new(pool.clone(), support::test_config());
    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    sqlx::query("UPDATE sandboxes SET state = 'active' WHERE id = $1")
        .bind(&setup.chief_computer_id)
        .execute(&pool)
        .await
        .unwrap();
    complete_target_and_resume(&pool, &setup.target_run_id, &setup.delegation_id).await;

    let resume_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE request_id LIKE 'delegation-return:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resume_count, 1);

    let user_message: String = sqlx::query_scalar(
        "SELECT user_message FROM work_queue WHERE provenance_kind = 'delegation_return'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(user_message.contains("notes.md"));
    assert!(
        user_message.contains("destination computer is no longer available")
            || user_message.contains("unavailable")
            || user_message.contains("failed")
    );
    assert!(!user_message.contains("available at "));
}

#[sqlx::test(migrations = "./migrations")]
async fn content_hash_conflict_is_fail_closed(pool: PgPool) {
    let setup = cross_computer_file_delegation(&pool, "hash-conflict").await;
    let dest = Arc::new(FakeAgentComputer::new());
    dest.write_file(&setup.destination_path, b"different bytes")
        .await
        .unwrap();
    let state = AppState::new(pool.clone(), support::test_config());
    state
        .computer_registry
        .register_test_computer("alice", &setup.chief_computer_id, dest);

    artifact_handoff::execute_pending_transfers_for_delegation(&state, &setup.delegation_id)
        .await
        .unwrap();

    let (status, code): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_code FROM delegation_artifact_transfers WHERE delegation_id = $1",
    )
    .bind(&setup.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(code.as_deref(), Some("destination_conflict"));
}
