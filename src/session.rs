use crate::settings::SessionSettings;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: String,
    pub content_summary: String,
    pub model_used: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvent {
    pub id: Uuid,
    pub tool: String,
    pub summary: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub client_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub tool_history: Vec<ToolEvent>,
    pub settings: SessionSettings,
}

impl Session {
    pub fn new(client_id: Option<String>, settings: SessionSettings) -> Self {
        Self {
            id: Uuid::new_v4(),
            client_id,
            created_at: Utc::now(),
            messages: Vec::new(),
            tool_history: Vec::new(),
            settings,
        }
    }
}


