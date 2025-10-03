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
#[serde(rename_all = "snake_case")]
pub enum WebSearchStatus {
    NotUsed,
    InProgress,
    Searching,
    Completed,
}

impl Default for WebSearchStatus {
    fn default() -> Self {
        WebSearchStatus::NotUsed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub id: Uuid,
    pub role: ConversationRole,
    pub content: String,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub web_search_status: WebSearchStatus,
    pub timestamp: DateTime<Utc>,
}

impl ConversationEntry {
    pub fn new(role: ConversationRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            reasoning: None,
            web_search_status: WebSearchStatus::NotUsed,
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
        let txt_filename = format!("{}-conversation.txt", guard.session_id);
        let json_filename = format!("{}-conversation.json", guard.session_id);
        let txt_path = self.log_dir.join(txt_filename);
        let json_path = self.log_dir.join(json_filename);

        // Write plain text log
        let mut buffer = String::new();
        for entry in &guard.entries {
            buffer.push_str(&format!(
                "[{timestamp}] {role}:\n{content}\n",
                timestamp = entry.timestamp.to_rfc3339(),
                role = entry.role.label(),
                content = entry.content
            ));

            // Add reasoning if present
            if let Some(ref reasoning) = entry.reasoning {
                if !reasoning.is_empty() {
                    buffer.push_str(&format!("\n[Reasoning]\n{reasoning}\n"));
                }
            }

            // Add web search status if used
            match entry.web_search_status {
                WebSearchStatus::Completed => {
                    buffer.push_str("[Web Search: Completed]\n");
                }
                WebSearchStatus::InProgress | WebSearchStatus::Searching => {
                    buffer.push_str("[Web Search: In Progress]\n");
                }
                WebSearchStatus::NotUsed => {}
            }

            buffer.push_str("\n");
        }
        fs::write(&txt_path, buffer)
            .with_context(|| format!("failed to write conversation log to {}", txt_path.display()))?;

        // Write JSON log (structured)
        let json_data = serde_json::json!({
            "session_id": guard.session_id,
            "entries": guard.entries,
            "entry_count": guard.entries.len(),
        });
        let json_content = serde_json::to_string_pretty(&json_data)
            .context("failed to serialize conversation to JSON")?;
        fs::write(&json_path, json_content)
            .with_context(|| format!("failed to write JSON log to {}", json_path.display()))?;

        Ok(txt_path)
    }
}
