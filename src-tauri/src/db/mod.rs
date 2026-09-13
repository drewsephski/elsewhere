mod schema;

use crate::error::AppError;
use crate::models::{
    Bot, Conversation, CreateBotInput, Message, MessageRole, MessageStatus, UpdateBotInput,
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
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn now_ms() -> i64 {
        Utc::now().timestamp_millis()
    }

    pub fn list_bots(&self, include_archived: bool) -> Result<Vec<Bot>, AppError> {
        let sql = if include_archived {
            "SELECT id, name, description, system_prompt, provider, model, created_at, updated_at, archived_at FROM bots ORDER BY updated_at DESC"
        } else {
            "SELECT id, name, description, system_prompt, provider, model, created_at, updated_at, archived_at FROM bots WHERE archived_at IS NULL ORDER BY updated_at DESC"
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
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                archived_at: row.get(8)?,
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
            "SELECT id, name, description, system_prompt, provider, model, created_at, updated_at, archived_at FROM bots WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| {
            Ok(Bot {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                system_prompt: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                archived_at: row.get(8)?,
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
        let provider = input
            .provider
            .unwrap_or_else(|| "openai".to_string());
        let system_prompt = input.system_prompt.unwrap_or_default();
        self.conn.execute(
            "INSERT INTO bots (id, name, description, system_prompt, provider, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                name,
                input.description,
                system_prompt,
                provider,
                input.model.trim(),
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
        let system_prompt = input
            .system_prompt
            .unwrap_or(existing.system_prompt);
        let model = input
            .model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or(existing.model);
        let now = Self::now_ms();
        self.conn.execute(
            "UPDATE bots SET name = ?1, description = ?2, system_prompt = ?3, model = ?4, updated_at = ?5 WHERE id = ?6",
            params![name, description, system_prompt, model, now, input.id],
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
        let deleted = self.conn.execute("DELETE FROM bots WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Err(AppError::NotFound(format!("bot {}", id)));
        }
        Ok(())
    }

    pub fn get_or_create_primary_conversation(&self, bot_id: &str) -> Result<Conversation, AppError> {
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
            "SELECT id, conversation_id, role, kind, body, status, model, error_message, created_at, updated_at FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
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
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, kind, body, status, model, created_at, updated_at) VALUES (?1, ?2, ?3, 'text', ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                conversation_id,
                role.as_str(),
                body,
                status.as_str(),
                model,
                now,
                now
            ],
        )?;
        self.touch_conversation(conversation_id)?;
        self.get_message(&id)
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
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("message {}", id))
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateBotInput;

    #[test]
    fn bot_persistence_roundtrip() {
        let db = Database::open_in_memory().expect("db");
        let bot = db
            .create_bot(CreateBotInput {
                name: "Researcher".into(),
                description: None,
                system_prompt: Some("You are helpful.".into()),
                provider: Some("openai".into()),
                model: "gpt-4o-mini".into(),
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
                model: "gpt-4o-mini".into(),
            })
            .expect("create");
        let updated = db
            .update_bot(UpdateBotInput {
                id: bot.id.clone(),
                name: None,
                description: None,
                system_prompt: None,
                model: None,
            })
            .expect("update");
        assert_eq!(updated.name, "Researcher");
        assert_eq!(updated.description.as_deref(), Some("Notes"));
        assert_eq!(updated.system_prompt, "You are helpful.");
        assert_eq!(updated.model, "gpt-4o-mini");
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
                model: "gpt-4o-mini".into(),
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
                model: "gpt-4o-mini".into(),
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
                Some("gpt-4o-mini"),
            )
            .expect("assistant");
        db.append_message_body(&assistant.id, "world").expect("append");
        db
            .update_message_body_and_status(
                &assistant.id,
                "world",
                MessageStatus::Complete,
                None,
            )
            .expect("complete");
        let messages = db.list_messages(&conv.id).expect("list");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, user.id);
        assert_eq!(messages[1].body, "world");
    }
}
