use rusqlite::Connection;

const CURRENT_SCHEMA_VERSION: i32 = 2;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );
        ",
    )?;

    let version = current_version(conn)?;
    if version == 0 {
        migrate_to_v1(conn)?;
        set_version(conn, 1)?;
    }
    if current_version(conn)? < 2 {
        migrate_to_v2(conn)?;
        set_version(conn, 2)?;
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
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
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
    let has_sequence: bool = conn
        .prepare("SELECT sequence FROM messages LIMIT 1")
        .is_ok();

    if !has_sequence {
        conn.execute_batch(
            "
            ALTER TABLE messages ADD COLUMN sequence INTEGER;
            ",
        )?;

        conn.execute_batch(
            "
            UPDATE messages
            SET sequence = (
                SELECT COUNT(*) FROM messages AS older
                WHERE older.conversation_id = messages.conversation_id
                  AND (
                    older.created_at < messages.created_at
                    OR (older.created_at = messages.created_at AND older.id <= messages.id)
                  )
            );
            ",
        )?;

        conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_messages_conversation_sequence
                ON messages(conversation_id, sequence ASC);
            ",
        )?;
    }

    Ok(())
}
