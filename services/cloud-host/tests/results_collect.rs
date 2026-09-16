use agent_core::{AgentComputer, ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
use async_trait::async_trait;
use cloud_host::results;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

struct ListingComputer {
    listings: Mutex<HashMap<String, Vec<WorkspaceEntry>>>,
    files: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl AgentComputer for ListingComputer {
    async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
        Ok(ComputerInfo {
            ready: true,
            protocol_version: 1,
            detail: None,
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<WorkspaceEntry>, ComputerError> {
        Ok(self
            .listings
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .unwrap_or_default())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, ComputerError> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| ComputerError::ExecutionFailed(format!("missing {path}")))
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ComputerError> {
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
async fn collect_persists_summary_and_result_files(pool: PgPool) {
    let run_id = Uuid::new_v4().to_string();
    let bot_id = Uuid::new_v4().to_string();
    let computer_id = Uuid::new_v4().to_string();
    let conversation_id = Uuid::new_v4().to_string();
    let assistant_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO sandboxes (id, owner_id, display_name, provider, provider_resource_id, state) VALUES ($1, 'alice', 'c', 'fly_sprite', 'sprite', 'active')",
    )
    .bind(&computer_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO bots (id, owner_id, name, system_prompt, model, computer_id) VALUES ($1, 'alice', 'Designer', '', 'gpt-5.6-luna', $2)",
    )
    .bind(&bot_id)
    .bind(&computer_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'alice', $2)")
        .bind(&conversation_id)
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, body, status, sequence) VALUES ($1, $2, 'assistant', 'Done with deliverables', 'complete', 1)",
    )
    .bind(&assistant_id)
    .bind(&conversation_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agent_runs (id, owner_id, request_id, bot_id, conversation_id, computer_id, model, status, assistant_message_id, finished_at) VALUES ($1, 'alice', $2, $3, $4, $5, 'gpt-5.6-luna', 'completed', $6, NOW())",
    )
    .bind(&run_id)
    .bind(&request_id)
    .bind(&bot_id)
    .bind(&conversation_id)
    .bind(&computer_id)
    .bind(&assistant_id)
    .execute(&pool)
    .await
    .unwrap();

    let results_dir = results::output_directory(&run_id);
    let computer = ListingComputer {
        listings: Mutex::new(HashMap::from([
            (
                "/workspace".to_string(),
                vec![WorkspaceEntry {
                    name: "results".into(),
                    path: "/workspace/results".into(),
                    is_dir: true,
                }],
            ),
            (
                "/workspace/results".to_string(),
                vec![WorkspaceEntry {
                    name: run_id.clone(),
                    path: format!("/workspace/results/{run_id}"),
                    is_dir: true,
                }],
            ),
            (
                results_dir.clone(),
                vec![
                    WorkspaceEntry {
                        name: "hello_world".into(),
                        path: format!("{results_dir}/hello_world"),
                        is_dir: false,
                    },
                    WorkspaceEntry {
                        name: "shot.png".into(),
                        path: format!("{results_dir}/shot.png"),
                        is_dir: false,
                    },
                ],
            ),
        ])),
        files: Mutex::new(HashMap::from([
            (
                format!("{results_dir}/hello_world"),
                b"hello from elsewhere".to_vec(),
            ),
            (
                format!("{results_dir}/shot.png"),
                vec![0x89, 0x50, 0x4e, 0x47],
            ),
        ])),
    };

    results::collect(&pool, &run_id, &computer)
        .await
        .expect("collect should succeed");

    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM work_results WHERE run_id = $1 ORDER BY name")
            .bind(&run_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(names.contains(&"summary.md".to_string()));
    assert!(names.contains(&"hello_world".to_string()));
    assert!(names.contains(&"shot.png".to_string()));
}
