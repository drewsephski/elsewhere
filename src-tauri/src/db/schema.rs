use rusqlite::Connection;

const CURRENT_SCHEMA_VERSION: i32 = 5;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );
        ",
    )?;

    let version = current_version(conn)?;
    if version < 1 {
        run_migration(conn, 1, migrate_to_v1)?;
    }
    if current_version(conn)? < 2 {
        run_migration(conn, 2, migrate_to_v2)?;
    }
    if current_version(conn)? < 3 {
        run_migration(conn, 3, migrate_to_v3)?;
    }
    if current_version(conn)? < 4 {
        run_migration(conn, 4, migrate_to_v4)?;
    }
    if current_version(conn)? < 5 {
        run_migration(conn, 5, migrate_to_v5)?;
    }

    let final_version = current_version(conn)?;
    if final_version != CURRENT_SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidColumnType(
            0,
            "schema_version".into(),
            rusqlite::types::Type::Integer,
        ));
    }

    Ok(())
}

fn run_migration(
    conn: &Connection,
    target_version: i32,
    migrate: fn(&Connection) -> Result<(), rusqlite::Error>,
) -> Result<(), rusqlite::Error> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        migrate(conn)?;
        set_version(conn, target_version)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
        return result;
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
}

fn current_version(conn: &Connection) -> Result<i32, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT version FROM schema_version LIMIT 1")?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        return row.get(0);
    }
    Ok(0)
}

fn set_version(conn: &Connection, version: i32) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        [version],
    )?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_to_v1(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bots (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            system_prompt TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            archived_at INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_bots_updated ON bots(updated_at DESC);

        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY NOT NULL,
            bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
            title TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_conversations_bot ON conversations(bot_id, updated_at DESC);

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'text',
            body TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'complete',
            model TEXT,
            error_message TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at ASC);
        ",
    )?;
    Ok(())
}

fn migrate_to_v2(conn: &Connection) -> Result<(), rusqlite::Error> {
    if !column_exists(conn, "messages", "sequence")? {
        conn.execute_batch(
            "
            ALTER TABLE messages ADD COLUMN sequence INTEGER;
            ",
        )?;
    }

    conn.execute_batch(
        "
        UPDATE messages
        SET sequence = (
            SELECT COUNT(*) FROM messages AS older
            WHERE older.conversation_id = messages.conversation_id
              AND (
                older.created_at < messages.created_at
                OR (older.created_at = messages.created_at AND older.id < messages.id)
              )
        )
        WHERE sequence IS NULL;
        ",
    )?;

    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_messages_conversation_sequence
            ON messages(conversation_id, sequence ASC);
        ",
    )?;

    Ok(())
}

fn migrate_to_v5(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn migrate_to_v4(conn: &Connection) -> Result<(), rusqlite::Error> {
    if !column_exists(conn, "bots", "computer_enabled")? {
        conn.execute_batch(
            "
            ALTER TABLE bots ADD COLUMN computer_enabled INTEGER NOT NULL DEFAULT 0;
            UPDATE bots SET computer_enabled = 1;
            UPDATE bots SET computer_enabled = 0 WHERE name = 'Scout';
            ",
        )?;
    }

    if !column_exists(conn, "agent_runs", "bot_id")? {
        conn.execute_batch(
            "
            ALTER TABLE agent_runs ADD COLUMN bot_id TEXT REFERENCES bots(id);
            ALTER TABLE agent_runs ADD COLUMN model TEXT;
            ALTER TABLE agent_runs ADD COLUMN computer_id TEXT;
            ALTER TABLE agent_runs ADD COLUMN started_at INTEGER;
            ALTER TABLE agent_runs ADD COLUMN finished_at INTEGER;
            ",
        )?;
    }

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS run_events (
            id TEXT PRIMARY KEY NOT NULL,
            run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
            sequence INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_run_events_run
            ON run_events(run_id, sequence ASC);
        ",
    )?;

    Ok(())
}

fn migrate_to_v3(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS agent_runs (
            id TEXT PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            request_id TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL,
            error_code TEXT,
            step_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_agent_runs_conversation
            ON agent_runs(conversation_id, created_at DESC);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_backfill_sequence_uses_strict_id_order_at_equal_timestamps() {
        let conn = Connection::open_in_memory().expect("conn");
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")
            .expect("schema_version");
        run_migration(&conn, 1, migrate_to_v1).expect("v1");

        conn.execute_batch(
            "
            INSERT INTO bots (id, name, system_prompt, model, created_at, updated_at)
            VALUES ('bot-1', 'Bot', '', 'gpt-5.6-luna', 1, 1);
            INSERT INTO conversations (id, bot_id, created_at, updated_at)
            VALUES ('conv-1', 'bot-1', 1, 1);
            INSERT INTO messages (id, conversation_id, role, kind, body, status, created_at, updated_at)
            VALUES
              ('msg-a', 'conv-1', 'user', 'text', 'first', 'complete', 100, 100),
              ('msg-b', 'conv-1', 'assistant', 'text', 'second', 'complete', 100, 100);
            ",
        )
        .expect("seed");

        run_migration(&conn, 2, migrate_to_v2).expect("v2");

        let seq_a: i64 = conn
            .query_row(
                "SELECT sequence FROM messages WHERE id = 'msg-a'",
                [],
                |row| row.get(0),
            )
            .expect("seq a");
        let seq_b: i64 = conn
            .query_row(
                "SELECT sequence FROM messages WHERE id = 'msg-b'",
                [],
                |row| row.get(0),
            )
            .expect("seq b");

        assert_eq!(seq_a, 0);
        assert_eq!(seq_b, 1);
    }
}
