use agent_core::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use async_trait::async_trait;
use cloud_host::{
    bot_context::{self, ContextInput},
    db::resources,
    results, work,
};
use sqlx::PgPool;
use std::sync::Mutex;

async fn bot(pool: &PgPool) -> resources::BotRow {
    let computer = resources::insert_computer_placeholder(pool, "alice", "Computer")
        .await
        .unwrap();
    resources::insert_bot(
        pool,
        "alice",
        "Scout",
        "Be helpful",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}
#[sqlx::test(migrations = "./migrations")]
async fn context_is_private_versioned_and_snapshotted(pool: PgPool) {
    let bot = bot(&pool).await;
    assert!(bot_context::get(&pool, "bob", &bot.id).await.is_err());
    assert!(bot_context::save(
        &pool,
        "bob",
        &bot.id,
        ContextInput {
            content: "attack".into(),
            revision: 0
        }
    )
    .await
    .is_err());
    let saved = bot_context::save(
        &pool,
        "alice",
        &bot.id,
        ContextInput {
            content: "Cite primary sources".into(),
            revision: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision, 1);
    assert!(bot_context::save(
        &pool,
        "alice",
        &bot.id,
        ContextInput {
            content: "stale".into(),
            revision: 0
        }
    )
    .await
    .is_err());
    let first = work::enqueue(&pool, "alice", "first", &bot.id, None, "Research")
        .await
        .unwrap();
    bot_context::save(
        &pool,
        "alice",
        &bot.id,
        ContextInput {
            content: "Use a table".into(),
            revision: 1,
        },
    )
    .await
    .unwrap();
    let replay = work::enqueue(&pool, "alice", "first", &bot.id, None, "Research")
        .await
        .unwrap();
    assert_eq!(first.instructions, replay.instructions);
    assert!(first.instructions.contains("Cite primary sources"));
    let next = work::enqueue(&pool, "alice", "next", &bot.id, None, "Research")
        .await
        .unwrap();
    assert!(next.instructions.contains("Use a table"));
    assert!(!next.instructions.contains("Cite primary sources"));
    assert!(bot_context::save(
        &pool,
        "alice",
        &bot.id,
        ContextInput {
            content: "x".repeat(16001),
            revision: 2
        }
    )
    .await
    .is_err());
}
struct Files {
    run_id: String,
    reads: Mutex<Vec<String>>,
}
#[async_trait]
impl AgentComputer for Files {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        unreachable!()
    }
    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        let entries = if path == "/workspace" {
            vec![("results", true)]
        } else if path == "/workspace/results" {
            vec![(self.run_id.as_str(), true)]
        } else {
            vec![
                ("report.md", false),
                ("too-large.bin", false),
                ("../escape", false),
                ("folder", true),
            ]
        };
        Ok(entries
            .into_iter()
            .map(|(name, is_dir)| WorkspaceEntry {
                name: name.into(),
                path: "/etc/host-secret".into(),
                is_dir,
            })
            .collect())
    }
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        self.reads.lock().unwrap().push(path.into());
        if path.ends_with("too-large.bin") {
            Ok(vec![0; results::FILE_LIMIT + 1])
        } else {
            Ok(b"Actual computer file".to_vec())
        }
    }
    async fn write_file(&self, _: &str, _: &[u8]) -> Result<(), ComputerError> {
        panic!("collection must not write")
    }
    async fn exec(&self, _: &str) -> Result<ExecResult, ComputerError> {
        panic!("collection must not execute")
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn completed_files_are_bounded_immutable_and_computer_stays_reserved(pool: PgPool) {
    let bot = bot(&pool).await;
    let run = work::enqueue(&pool, "alice", "files", &bot.id, None, "Create report")
        .await
        .unwrap();
    work::claim_next(&pool).await.unwrap().unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed' WHERE id = $1")
        .bind(&run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE messages SET body = 'A finished summary', status = 'complete' WHERE id = $1",
    )
    .bind(&run.assistant_message_id)
    .execute(&pool)
    .await
    .unwrap();
    let next = work::enqueue(&pool, "alice", "next", &bot.id, None, "Next report")
        .await
        .unwrap();
    assert!(
        work::claim_next(&pool).await.unwrap().is_none(),
        "still collecting on this computer"
    );
    let computer = Files {
        run_id: run.run_id.clone(),
        reads: Mutex::new(vec![]),
    };
    assert!(results::collect(&pool, &run.run_id, &computer)
        .await
        .unwrap_err()
        .contains("Some files remain"));
    let rows: Vec<(String, Vec<u8>)> =
        sqlx::query_as("SELECT name, content FROM work_results WHERE run_id = $1 ORDER BY name")
            .bind(&run.run_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            ("report.md".into(), b"Actual computer file".to_vec()),
            ("summary.md".into(), b"A finished summary".to_vec())
        ]
    );
    assert!(computer.reads.lock().unwrap().iter().all(|path| path
        .starts_with(&results::output_directory(&run.run_id))
        && !path.contains("..")));
    results::save(&pool, &run.run_id, "report.md", "file", b"Replacement")
        .await
        .unwrap();
    let body: Vec<u8> = sqlx::query_scalar(
        "SELECT content FROM work_results WHERE run_id = $1 AND name = 'report.md'",
    )
    .bind(&run.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(body, b"Actual computer file");
    cloud_host::db::queries::mark_interrupted_runs(&pool)
        .await
        .unwrap();
    assert_eq!(
        work::claim_next(&pool)
            .await
            .unwrap()
            .unwrap()
            .records
            .run_id,
        next.run_id
    );
}
