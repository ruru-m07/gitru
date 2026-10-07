//! Typed provider observations; native participant state is not commit approval.
use crate::{
    checks::CheckV1,
    reviews::{ReviewThreadV1, ReviewV1},
    tasks::TaskV1,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum NativeDetailPayload {
    #[serde(rename = "participant.v1")]
    ParticipantV1(ParticipantV1),
    #[serde(rename = "task.v1")]
    TaskV1(TaskV1),
    #[serde(rename = "check.v1")]
    CheckV1(CheckV1),
    #[serde(rename = "review.v1")]
    ReviewV1(ReviewV1),
    #[serde(rename = "review_thread.v1")]
    ReviewThreadV1(ReviewThreadV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantUser {
    pub provider_id: String,
    pub login: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantV1 {
    pub user: ParticipantUser,
    pub role: Option<String>,
    pub approved: Option<bool>,
    pub state: Option<String>,
    pub participated_at: Option<String>,
}
