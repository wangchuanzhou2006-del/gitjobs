use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TalentKind {
    Hiring,
    Seeking,
    Irrelevant,
    #[default]
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImportItemStatus {
    Queued,
    Extracting,
    NeedsReview,
    AutoPublished,
    Published,
    Rejected,
    Merged,
    Failed,
}

impl ImportItemStatus {
    pub(crate) const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Extracting)
                | (
                    Self::Extracting,
                    Self::NeedsReview | Self::AutoPublished | Self::Failed
                )
                | (
                    Self::NeedsReview,
                    Self::Published | Self::Rejected | Self::Merged | Self::Queued
                )
                | (Self::Rejected | Self::Failed, Self::Queued)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceType {
    WechatGroup,
    QqGroup,
    Web,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceTrust {
    New,
    Trusted,
    Watchlist,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContactType {
    Phone,
    Wechat,
    Qq,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContactCandidate {
    pub contact_type: ContactType,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RiskReason {
    pub code: String,
    pub explanation: String,
    pub evidence: Option<String>,
    pub hard_block: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DuplicateCandidate {
    pub candidate_item_id: Option<Uuid>,
    pub candidate_record_id: Option<Uuid>,
    pub score: f32,
    pub reasons: Vec<String>,
    pub high_similarity: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TalentExtraction {
    pub kind: TalentKind,
    pub confidence: f32,
    pub title: Option<String>,
    pub scene: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub roles: Vec<String>,
    pub skills: Vec<String>,
    pub experience_years: Option<u8>,
    pub hours_per_day: Option<u8>,
    pub rest_days_per_month: Option<u8>,
    pub lodging: Option<String>,
    pub housing_allowance: Option<u32>,
    pub availability: Option<String>,
    pub urgency: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub contacts: Vec<ContactCandidate>,
    pub missing_fields: Vec<String>,
    pub risk_candidates: Vec<RiskReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextFragment {
    pub position: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub text: String,
}
