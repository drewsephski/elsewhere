mod schema;

use crate::error::AppError;
use crate::models::{
    Bot, Conversation, CreateBotInput, Message, MessageRole, MessageStatus, UpdateBotInput,
    DEFAULT_MODEL,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Other(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        schema::migrate(&conn)?;
        let db = Self { conn };
        db.recover_interrupted_messages()?;
        db.ensure_demo_agent()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        schema::migrate(&conn)?;
        let db = Self { conn };
        db.recover_interrupted_messages()?;
        db.ensure_demo_agent()?;
        Ok(db)
    }

    /// Marks in-flight assistant generations from a prior session as interrupted.
    pub fn recover_interrupted_messages(&self) -> Result<(), AppError> {
        let now = Self::now_ms();
        self.conn.execute(
            "UPDATE messages SET status = ?1, updated_at = ?2 WHERE status IN ('pending', 'streaming')",
            params![MessageStatus::Interrupted.as_str(), now],
        )?;
        self.conn.execute(
            "UPDATE agent_runs SET status = 'interrupted', updated_at = ?1 WHERE status = 'running'",
            params![now],
        )?;
        Ok(())
    }

    fn next_message_sequence(&self, conversation_id: &str) -> Result<i64, AppError> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(sequence), -1) + 1 FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(AppError::Database)
    }

    fn now_ms() -> i64 {
        Utc::now().timestamp_millis()
    }

    pub fn ensure_demo_agent(&self) -> Result<(), AppError> {
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM bots WHERE name = ?1 AND archived_at IS NULL",
                params!["Scout"],
                |row| row.get(0),
            )
            .map_err(AppError::Database)?;
        if exists > 0 {
            return Ok(());
        }

        let system_prompt = "You are Scout, a calm and capable research assistant inside Elsewhere.\n\n\
Help the user explore ideas, compare options, and turn curiosity into clear next steps. \
Be concise unless they ask for depth. Prefer structured answers with short headings and bullet points when useful.\n\n\
When information might be outdated, say what you know and what you would verify. Never invent sources.";

        self.create_bot(CreateBotInput {
            name: "Scout".to_string(),
            description: Some("Research & discovery".to_string()),
            system_prompt: Some(system_prompt.to_string()),
            provider: None,
            model: DEFAULT_MODEL.to_string(),
            computer_enabled: Some(false),
        })?;
        Ok(())
    }

    pub fn list_bots(&self, include_archived: bool) -> Result<Vec<Bot>, AppError> {
        let sql = if include_archived {
            "SELECT id, name, description, system_prompt, provider, model, computer_enabled, created_at, updated_at, archived_at FROM bots ORDER BY updated_at DESC"
        } else {
            "SELECT id, name, description, system_prompt, provider, model, computer_enabled, created_at, updated_at, archived_at FROM bots WHERE archived_at IS NULL ORDER BY updated_at DESC"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(Bot {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                system_prompt: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                computer_enabled: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                archived_at: row.get(9)?,
            })
        })?;
        let mut bots = Vec::new();
        for bot in rows {
            bots.push(bot?);
        }
        Ok(bots)
    }

    pub fn get_bot(&self, id: &str) -> Result<Bot, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, system_prompt, provider, model, computer_enabled, created_at, updated_at, archived_at FROM bots WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| {
            Ok(Bot {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                system_prompt: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                computer_enabled: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                archived_at: row.get(9)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("bot {}", id)),
            other => AppError::Database(other),
        })
    }

    pub fn create_bot(&self, input: CreateBotInput) -> Result<Bot, AppError> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("bot name is required".into()));
        }
        if input.model.trim().is_empty() {
            return Err(AppError::Validation("model is required".into()));
        }
        let now = Self::now_ms();
        let id = Uuid::new_v4().to_string();
        let provider = input.provider.unwrap_or_else(|| "openai".to_string());
        let system_prompt = input.system_prompt.unwrap_or_default();
        let computer_enabled = if input.computer_enabled.unwrap_or(true) {
            1
        } else {
            0
        };
        self.conn.execute(
            "INSERT INTO bots (id, name, description, system_prompt, provider, model, computer_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                name,
                input.description,
                system_prompt,
                provider,
                input.model.trim(),
                computer_enabled,
                now,
                now
            ],
        )?;
        self.get_bot(&id)
    }

    pub fn update_bot(&self, input: UpdateBotInput) -> Result<Bot, AppError> {
        let existing = self.get_bot(&input.id)?;
        let name = input
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let system_prompt = input.system_prompt.unwrap_or(existing.system_prompt);
        let model = input
            .model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or(existing.model);
        let computer_enabled = input.computer_enabled.unwrap_or(existing.computer_enabled);
        let computer_enabled_int = if computer_enabled { 1 } else { 0 };
        let now = Self::now_ms();
        self.conn.execute(
            "UPDATE bots SET name = ?1, description = ?2, system_prompt = ?3, model = ?4, computer_enabled = ?5, updated_at = ?6 WHERE id = ?7",
            params![name, description, system_prompt, model, computer_enabled_int, now, input.id],
        )?;
        self.get_bot(&input.id)
    }

    pub fn touch_bot(&self, id: &str) -> Result<(), AppError> {
        let now = Self::now_ms();
        let updated = self.conn.execute(
            "UPDATE bots SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("bot {}", id)));
        }
        Ok(())
    }

    pub fn archive_bot(&self, id: &str) -> Result<(), AppError> {
        let now = Self::now_ms();
        let updated = self
            .conn
            .execute(
                "UPDATE bots SET archived_at = ?1, updated_at = ?2 WHERE id = ?3 AND archived_at IS NULL",
                params![now, now, id],
            )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("bot {}", id)));
        }
        Ok(())
    }

    pub fn delete_bot(&self, id: &str) -> Result<(), AppError> {
        let deleted = self
            .conn
            .execute("DELETE FROM bots WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Err(AppError::NotFound(format!("bot {}", id)));
        }
        Ok(())
    }

    pub fn get_or_create_primary_conversation(
        &self,
        bot_id: &str,
    ) -> Result<Conversation, AppError> {
        self.get_bot(bot_id)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, bot_id, title, created_at, updated_at FROM conversations WHERE bot_id = ?1 ORDER BY updated_at DESC LIMIT 1",
        )?;
        let existing = stmt.query_row(params![bot_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                bot_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        });
        if let Ok(conv) = existing {
            return Ok(conv);
        }
        self.create_conversation(bot_id, None)
    }

    pub fn create_conversation(
        &self,
        bot_id: &str,
        title: Option<String>,
    ) -> Result<Conversation, AppError> {
        self.get_bot(bot_id)?;
        let now = Self::now_ms();
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO conversations (id, bot_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, bot_id, title, now, now],
        )?;
        Ok(Conversation {
            id,
            bot_id: bot_id.to_string(),
            title,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn list_conversations(&self, bot_id: &str) -> Result<Vec<Conversation>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, bot_id, title, created_at, updated_at FROM conversations WHERE bot_id = ?1 ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![bot_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                bot_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for c in rows {
            list.push(c?);
        }
        Ok(list)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, bot_id, title, created_at, updated_at FROM conversations WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                bot_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("conversation {}", id))
            }
            other => AppError::Database(other),
        })
    }

    pub fn get_conversation_for_bot(
        &self,
        conversation_id: &str,
        bot_id: &str,
    ) -> Result<Conversation, AppError> {
        let conversation = self.get_conversation(conversation_id)?;
        if conversation.bot_id != bot_id {
            return Err(AppError::Validation(
                "conversation does not belong to this bot".into(),
            ));
        }
        Ok(conversation)
    }

    pub fn touch_conversation(&self, id: &str) -> Result<(), AppError> {
        let now = Self::now_ms();
        let updated = self.conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("conversation {}", id)));
        }
        Ok(())
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<Message>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, kind, body, status, model, error_message, created_at, updated_at FROM messages WHERE conversation_id = ?1 ORDER BY sequence ASC, created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            let role_str: String = row.get(2)?;
            let status_str: String = row.get(5)?;
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: MessageRole::from_str(&role_str).unwrap_or(MessageRole::User),
                kind: row.get(3)?,
                body: row.get(4)?,
                status: MessageStatus::from_str(&status_str).unwrap_or(MessageStatus::Complete),
                model: row.get(6)?,
                error_message: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        let mut list = Vec::new();
        for m in rows {
            list.push(m?);
        }
        Ok(list)
    }

    pub fn insert_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        body: &str,
        status: MessageStatus,
        model: Option<&str>,
    ) -> Result<Message, AppError> {
        self.get_conversation(conversation_id)?;
        let now = Self::now_ms();
        let sequence = self.next_message_sequence(conversation_id)?;
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, kind, body, status, model, sequence, created_at, updated_at) VALUES (?1, ?2, ?3, 'text', ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                conversation_id,
                role.as_str(),
                body,
                status.as_str(),
                model,
                sequence,
                now,
                now
            ],
        )?;
        self.touch_conversation(conversation_id)?;
        self.get_message(&id)
    }

    pub fn insert_structured_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        kind: &str,
        body: &str,
        status: MessageStatus,
        model: Option<&str>,
    ) -> Result<Message, AppError> {
        self.get_conversation(conversation_id)?;
        let now = Self::now_ms();
        let sequence = self.next_message_sequence(conversation_id)?;
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, kind, body, status, model, sequence, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                conversation_id,
                role.as_str(),
                kind,
                body,
                status.as_str(),
                model,
                sequence,
                now,
                now
            ],
        )?;
        self.touch_conversation(conversation_id)?;
        self.get_message(&id)
    }

    pub fn create_agent_run(
        &self,
        conversation_id: &str,
        request_id: &str,
        bot_id: &str,
        model: &str,
        computer_id: Option<&str>,
    ) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now_ms();
        self.conn.execute(
            "INSERT INTO agent_runs (id, conversation_id, request_id, bot_id, model, computer_id, status, step_count, started_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running', 0, ?7, ?7, ?7)",
            params![id, conversation_id, request_id, bot_id, model, computer_id, now],
        )?;
        Ok(id)
    }

    pub fn append_run_event(
        &self,
        request_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<i64, AppError> {
        let run_id: String = self
            .conn
            .query_row(
                "SELECT id FROM agent_runs WHERE request_id = ?1",
                params![request_id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("agent run {}", request_id))
                }
                other => AppError::Database(other),
            })?;

        let sequence: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sequence), -1) + 1 FROM run_events WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(AppError::Database)?;

        let now = Self::now_ms();
        let id = Uuid::new_v4().to_string();
        let payload_json = payload.to_string();
        self.conn.execute(
            "INSERT INTO run_events (id, run_id, sequence, event_type, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, run_id, sequence, event_type, payload_json, now],
        )?;
        Ok(sequence)
    }

    pub fn update_agent_run(
        &self,
        request_id: &str,
        status: &str,
        error_code: Option<&str>,
        step_count: i64,
    ) -> Result<(), AppError> {
        let now = Self::now_ms();
        let finished_at: Option<i64> = if matches!(status, "completed" | "failed" | "cancelled") {
            Some(now)
        } else {
            None
        };
        let updated = self.conn.execute(
            "UPDATE agent_runs SET status = ?1, error_code = ?2, step_count = ?3, updated_at = ?4, finished_at = COALESCE(?5, finished_at) WHERE request_id = ?6",
            params![status, error_code, step_count, now, finished_at, request_id],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("agent run {}", request_id)));
        }
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Message, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, kind, body, status, model, error_message, created_at, updated_at FROM messages WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| {
            let role_str: String = row.get(2)?;
            let status_str: String = row.get(5)?;
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: MessageRole::from_str(&role_str).unwrap_or(MessageRole::User),
                kind: row.get(3)?,
                body: row.get(4)?,
                status: MessageStatus::from_str(&status_str).unwrap_or(MessageStatus::Complete),
                model: row.get(6)?,
                error_message: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("message {}", id)),
            other => AppError::Database(other),
        })
    }

    pub fn update_message_body_and_status(
        &self,
        id: &str,
        body: &str,
        status: MessageStatus,
        error_message: Option<&str>,
    ) -> Result<Message, AppError> {
        let now = Self::now_ms();
        let updated = self.conn.execute(
            "UPDATE messages SET body = ?1, status = ?2, error_message = ?3, updated_at = ?4 WHERE id = ?5",
            params![body, status.as_str(), error_message, now, id],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("message {}", id)));
        }
        self.get_message(id)
    }

    pub fn append_message_body(&self, id: &str, append: &str) -> Result<(), AppError> {
        let now = Self::now_ms();
        let updated = self.conn.execute(
            "UPDATE messages SET body = body || ?1, updated_at = ?2 WHERE id = ?3",
            params![append, now, id],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("message {}", id)));
        }
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM app_meta WHERE key = ?1")?;
        match stmt.query_row(params![key], |row| row.get::<_, String>(0)) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(AppError::Database(error)),
        }
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn installation_id(&self) -> Result<String, AppError> {
        if let Some(existing) = self.get_meta("installation_id")? {
            return Ok(existing);
        }
        let id = Uuid::new_v4().to_string();
        self.set_meta("installation_id", &id)?;
        Ok(id)
    }

    pub fn elsewhere_pairing_identity(&self) -> Result<Option<(String, String)>, AppError> {
        match (
            self.get_meta("elsewhere_node_id")?,
            self.get_meta("elsewhere_computer_id")?,
        ) {
            (Some(node_id), Some(computer_id)) => Ok(Some((node_id, computer_id))),
            _ => Ok(None),
        }
    }

    pub fn set_elsewhere_pairing_identity(
        &self,
        node_id: &str,
        computer_id: &str,
    ) -> Result<(), AppError> {
        self.set_meta("elsewhere_node_id", node_id)?;
        self.set_meta("elsewhere_computer_id", computer_id)?;
        Ok(())
    }

    pub fn this_mac_paused(&self) -> Result<bool, AppError> {
        Ok(self.get_meta("this_mac_paused")?.as_deref() == Some("1"))
    }

    pub fn set_this_mac_paused(&self, paused: bool) -> Result<(), AppError> {
        self.set_meta("this_mac_paused", if paused { "1" } else { "0" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateBotInput, DEFAULT_MODEL};

    #[test]
    fn bot_persistence_roundtrip() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Researcher".into(),
                description: None,
                system_prompt: Some("You are helpful.".into()),
                provider: Some("openai".into()),
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        let loaded = db.get_bot(&bot.id).expect("get");
        assert_eq!(loaded.name, "Researcher");
        let updated = db
            .update_bot(UpdateBotInput {
                id: bot.id.clone(),
                name: Some("Renamed".into()),
                description: None,
                system_prompt: None,
                model: Some("gpt-4o".into()),
                computer_enabled: None,
            })
            .expect("update");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.model, "gpt-4o");
    }

    #[test]
    fn update_bot_with_none_fields_preserves_existing_values() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Researcher".into(),
                description: Some("Notes".into()),
                system_prompt: Some("You are helpful.".into()),
                provider: Some("openai".into()),
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        let updated = db
            .update_bot(UpdateBotInput {
                id: bot.id.clone(),
                name: None,
                description: None,
                system_prompt: None,
                model: None,
                computer_enabled: None,
            })
            .expect("update");
        assert_eq!(updated.name, "Researcher");
        assert_eq!(updated.description.as_deref(), Some("Notes"));
        assert_eq!(updated.system_prompt, "You are helpful.");
        assert_eq!(updated.model, DEFAULT_MODEL);
    }

    #[test]
    fn touch_bot_updates_timestamp_only() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Bot".into(),
                description: None,
                system_prompt: Some("Stay.".into()),
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        db.touch_bot(&bot.id).expect("touch");
        let touched = db.get_bot(&bot.id).expect("get");
        assert_eq!(touched.name, bot.name);
        assert_eq!(touched.system_prompt, bot.system_prompt);
        assert_eq!(touched.model, bot.model);
        assert!(touched.updated_at >= bot.updated_at);
    }

    #[test]
    fn conversation_and_messages_persist() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Bot".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        let conv = db.create_conversation(&bot.id, None).expect("conv");
        let user = db
            .insert_message(
                &conv.id,
                MessageRole::User,
                "hello",
                MessageStatus::Complete,
                None,
            )
            .expect("user");
        let assistant = db
            .insert_message(
                &conv.id,
                MessageRole::Assistant,
                "",
                MessageStatus::Streaming,
                Some(DEFAULT_MODEL),
            )
            .expect("assistant");
        db.append_message_body(&assistant.id, "world")
            .expect("append");
        db.update_message_body_and_status(&assistant.id, "world", MessageStatus::Complete, None)
            .expect("complete");
        let messages = db.list_messages(&conv.id).expect("list");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, user.id);
        assert_eq!(messages[1].body, "world");
    }

    #[test]
    fn messages_with_equal_timestamps_order_by_sequence() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Bot".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        let conv = db.create_conversation(&bot.id, None).expect("conv");
        let user = db
            .insert_message(
                &conv.id,
                MessageRole::User,
                "first",
                MessageStatus::Complete,
                None,
            )
            .expect("user");
        let assistant = db
            .insert_message(
                &conv.id,
                MessageRole::Assistant,
                "second",
                MessageStatus::Complete,
                None,
            )
            .expect("assistant");
        let messages = db.list_messages(&conv.id).expect("list");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, user.id);
        assert_eq!(messages[1].id, assistant.id);
    }

    #[test]
    fn new_bots_default_to_computer_enabled_scout_is_chat_only() {
        let db = Database::open_in_memory().expect("db");
        db.ensure_demo_agent().expect("demo");
        let scout = db
            .list_bots(false)
            .expect("list")
            .into_iter()
            .find(|b| b.name == "Scout")
            .expect("scout");
        assert!(!scout.computer_enabled);

        let worker = db
            .create_bot(CreateBotInput {
                name: "Worker".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        assert!(worker.computer_enabled);
    }

    #[test]
    fn ensure_demo_agent_creates_scout_on_open() {
        let db = Database::open_in_memory().expect("db");
        let bots = db.list_bots(false).expect("list");
        assert!(
            bots.iter().any(|bot| bot.name == "Scout"),
            "expected Scout demo agent after database open",
        );
    }

    #[test]
    fn recover_interrupted_messages_on_open() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Bot".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("create");
        let conv = db.create_conversation(&bot.id, None).expect("conv");
        let assistant = db
            .insert_message(
                &conv.id,
                MessageRole::Assistant,
                "partial",
                MessageStatus::Streaming,
                Some(DEFAULT_MODEL),
            )
            .expect("assistant");
        db.recover_interrupted_messages().expect("recover");
        let loaded = db.get_message(&assistant.id).expect("get");
        assert_eq!(loaded.status, MessageStatus::Interrupted);
    }

    #[test]
    fn get_conversation_for_bot_rejects_wrong_bot() {
        let db = Database::open_in_memory().expect("db");
        let bot_a = db
            .create_bot(CreateBotInput {
                name: "A".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("bot a");
        let bot_b = db
            .create_bot(CreateBotInput {
                name: "B".into(),
                description: None,
                system_prompt: None,
                provider: None,
                model: DEFAULT_MODEL.into(),
                computer_enabled: None,
            })
            .expect("bot b");
        let conv_a = db.create_conversation(&bot_a.id, None).expect("conv");
        let owned = db
            .get_conversation_for_bot(&conv_a.id, &bot_a.id)
            .expect("owned");
        assert_eq!(owned.id, conv_a.id);
        let err = db
            .get_conversation_for_bot(&conv_a.id, &bot_b.id)
            .expect_err("foreign");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn this_mac_pause_round_trips_in_meta() {
        let db = Database::open_in_memory().expect("db");
        assert!(!db.this_mac_paused().expect("read"));
        db.set_this_mac_paused(true).expect("write");
        assert!(db.this_mac_paused().expect("read"));
    }

    #[test]
    fn installation_id_is_stable_opaque_metadata() {
        let db = Database::open_in_memory().expect("db");
        let first = db.installation_id().expect("first");
        let second = db.installation_id().expect("second");
        assert_eq!(first, second);
        assert!(uuid::Uuid::parse_str(&first).is_ok());
    }
}
