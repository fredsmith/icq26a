use matrix_sdk::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buddy {
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub presence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub room_id: String,
    pub name: String,
    pub is_direct: bool,
    pub last_message: Option<String>,
    pub unread_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub room_id: String,
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub timestamp: u64,
    pub msg_type: String,
    pub media_url: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesPage {
    pub messages: Vec<Message>,
    pub end_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub homeserver: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub homeserver_url: String,
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEmoji {
    pub symbol: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvent {
    pub flow_id: String,
    pub user_id: String,
    pub is_self_verification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEmojisEvent {
    pub flow_id: String,
    pub user_id: String,
    pub emojis: Vec<VerificationEmoji>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
}

pub struct ServerLog {
    entries: std::sync::Mutex<Vec<LogEntry>>,
}

impl ServerLog {
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, level: &str, message: String) {
        let entry = LogEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            level: level.to_string(),
            message,
        };
        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);
        // Keep last 500 entries
        if entries.len() > 500 {
            let excess = entries.len() - 500;
            entries.drain(..excess);
        }
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedRoom {
    pub room_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub presence: String,
    pub last_seen_ago: Option<u64>,
    pub shared_rooms: Vec<SharedRoom>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomProfile {
    pub room_id: String,
    pub name: String,
    pub topic: Option<String>,
    pub is_direct: bool,
    pub member_count: u64,
}

pub fn data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or("Could not determine data directory")?;
    Ok(base.join("icq26a"))
}

pub fn session_file_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("session.json"))
}

pub struct MatrixState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub log: Arc<ServerLog>,
    pub sync_tasks: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl MatrixState {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            log: Arc::new(ServerLog::new()),
            sync_tasks: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn abort_sync_tasks(&self) {
        let mut tasks = self.sync_tasks.lock().unwrap();
        for task in tasks.drain(..) {
            task.abort();
        }
    }
}
