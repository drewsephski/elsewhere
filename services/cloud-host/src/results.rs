//! Immutable, bounded snapshots of a completed assignment, obtained only via AgentComputer.
use agent_core::AgentComputer;
use sqlx::PgPool;
use uuid::Uuid;

pub const FILE_LIMIT: usize = 1024 * 1024;
const TOTAL_LIMIT: usize = 5 * FILE_LIMIT;
const COUNT_LIMIT: usize = 20;

pub fn output_directory(run_id: &str) -> String {
    format!("/workspace/results/{run_id}")
}

pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
}

pub async fn save(
    pool: &PgPool,
    run_id: &str,
    name: &str,
    kind: &str,
    content: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO work_results (id, run_id, name, kind, content) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (run_id, kind, name) DO NOTHING")
        .bind(Uuid::new_v4()).bind(run_id).bind(name).bind(kind).bind(content).execute(pool).await?;
    Ok(())
}

/// Failed reads are visible as a collection note; completed work is never retried automatically.
pub async fn collect(
    pool: &PgPool,
    run_id: &str,
    computer: &dyn AgentComputer,
) -> Result<(), String> {
    let summary: Option<String> = sqlx::query_scalar("SELECT m.body FROM agent_runs r JOIN messages m ON m.id = r.assistant_message_id WHERE r.id = $1 AND r.status = 'completed'")
        .bind(run_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
    let Some(summary) = summary else {
        return Ok(());
    };
    if !summary.is_empty() && summary.len() <= FILE_LIMIT {
        save(pool, run_id, "summary.md", "summary", summary.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
    }
    let directory = output_directory(run_id);
    let _ = computer
        .ensure_ready()
        .await
        .map_err(|err| format!("The summary is saved. Could not prepare the computer: {err}"))?;
    // Missing output folder means no file deliverables; do not create or mutate the computer here.
    let root = computer.list_dir("/workspace").await.map_err(|_| {
        "The summary is saved. File results could not be checked on the computer.".to_string()
    })?;
    if !root
        .iter()
        .any(|entry| entry.is_dir && entry.name == "results")
    {
        return Ok(());
    }
    let parent = computer.list_dir("/workspace/results").await.map_err(|_| {
        "The summary is saved. File results could not be checked on the computer.".to_string()
    })?;
    let exists = parent
        .iter()
        .any(|entry| entry.is_dir && entry.name == run_id);
    if !exists {
        return Ok(());
    }
    let mut entries = computer
        .list_dir(&directory)
        .await
        .map_err(|_| "The summary is saved. File results could not be read.".to_string())?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let mut total = 0;
    let mut count = 0;
    let mut skipped = false;
    for entry in entries {
        if entry.is_dir || !safe_name(&entry.name) || count >= COUNT_LIMIT {
            skipped = true;
            continue;
        }
        // Never trust a provider-supplied absolute path, only a validated single-component filename.
        let path = format!("{directory}/{}", entry.name);
        match computer.read_file(&path).await {
            Ok(bytes) if bytes.len() <= FILE_LIMIT && total + bytes.len() <= TOTAL_LIMIT => {
                save(pool, run_id, &entry.name, "file", &bytes)
                    .await
                    .map_err(|e| e.to_string())?;
                total += bytes.len();
                count += 1;
            }
            _ => skipped = true,
        }
    }
    if skipped {
        return Err("Some files remain on the computer. Downloads include up to 20 top-level files, 1 MB each and 5 MB total; folders and unreadable files are skipped.".into());
    }
    Ok(())
}
