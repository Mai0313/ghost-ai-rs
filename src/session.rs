use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    System,
    User,
    Assistant,
    Reasoning,
    Error,
}

impl ConversationRole {
    pub fn label(&self) -> &str {
        match self {
            ConversationRole::System => "System",
            ConversationRole::User => "You",
            ConversationRole::Assistant => "Ghost",
            ConversationRole::Reasoning => "Reasoning",
            ConversationRole::Error => "Error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub id: Uuid,
    pub role: ConversationRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl ConversationEntry {
    pub fn new(role: ConversationRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Default)]
struct SessionState {
    session_id: Uuid,
    entries: Vec<ConversationEntry>,
}

impl SessionState {
    fn new_session() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            entries: Vec::new(),
        }
    }
}

pub struct SessionManager {
    log_dir: PathBuf,
    state: Mutex<SessionState>,
}

impl SessionManager {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        if !log_dir.exists() {
            fs::create_dir_all(&log_dir).context("failed to create log directory")?;
        }
        Ok(Self {
            log_dir,
            state: Mutex::new(SessionState::new_session()),
        })
    }

    pub fn current_session_id(&self) -> Uuid {
        self.state.lock().session_id
    }

    pub fn entries(&self) -> Vec<ConversationEntry> {
        self.state.lock().entries.clone()
    }

    pub fn append(&self, entry: ConversationEntry) {
        self.state.lock().entries.push(entry);
    }

    pub fn reset(&self) -> Uuid {
        let mut guard = self.state.lock();
        *guard = SessionState::new_session();
        guard.session_id
    }

    pub fn replace_all(&self, entries: Vec<ConversationEntry>) {
        let mut guard = self.state.lock();
        guard.entries = entries;
    }

    pub fn write_plaintext_log(&self) -> Result<PathBuf> {
        let guard = self.state.lock();
        let filename = format!("{}-conversation.txt", guard.session_id);
        let path = self.log_dir.join(filename);
        let mut buffer = String::new();
        for entry in &guard.entries {
            buffer.push_str(&format!(
                "[{timestamp}] {role}:\n{content}\n\n",
                timestamp = entry.timestamp.to_rfc3339(),
                role = entry.role.label(),
                content = entry.content
            ));
        }
        fs::write(&path, buffer)
            .with_context(|| format!("failed to write conversation log to {}", path.display()))?;
        Ok(path)
    }
}
