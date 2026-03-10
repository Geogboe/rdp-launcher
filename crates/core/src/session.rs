use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Launching,
    Active,
    Exited,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedSession {
    pub launch_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub target: String,
    pub process_id: u32,
    pub state: SessionState,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionHistoryEntry {
    pub launch_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub target: String,
    pub process_id: Option<u32>,
    pub state: SessionState,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
}
