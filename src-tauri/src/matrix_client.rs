use matrix_sdk::Client;
use serde::{Deserialize, Serialize};
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
    pub event_id: String,
    pub sender: String,
    pub sender_name: String,
    pub body: String,
    pub timestamp: u64,
    pub msg_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub homeserver: String,
    pub username: String,
    pub password: String,
}

pub struct MatrixState {
    pub client: Arc<Mutex<Option<Client>>>,
}

impl MatrixState {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }
}
