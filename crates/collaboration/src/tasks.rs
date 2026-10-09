//! Native task facts describe provider observations, never a remote write.
use crate::detail::{DetailValue, DetailValueState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActor {
    pub provider_id: String,
    pub kind: String,
    pub login: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskV1 {
    pub content: DetailValue,
    pub observed_content_state: DetailValueState,
    pub creator: TaskActor,
    pub state: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub pending: Option<bool>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<TaskActor>,
    pub comment_id: Option<String>,
}
