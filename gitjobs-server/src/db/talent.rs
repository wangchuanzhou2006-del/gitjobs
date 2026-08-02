//! Persistence boundary for encrypted talent imports and worker claims.

use std::fmt;

use anyhow::{Result, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::types::Json;
use tracing::{instrument, trace};
use uuid::Uuid;

use crate::{
    PgDB,
    talent::{
        ContactType, DuplicateCandidate, ImportItemStatus, RiskLevel, SourceTrust, SourceType,
        TalentExtraction, TalentKind, safe_extraction_contains_private_data,
    },
};

const MAX_TALENT_IMPORT_ITEMS: usize = 500;

#[derive(Clone, PartialEq)]
pub(crate) struct NewTalentImport {
    pub batch_id: Uuid,
    pub source_type: SourceType,
    pub source_name: String,
    pub imported_by: Option<Uuid>,
    pub raw_text_ciphertext: Vec<u8>,
    pub encryption_key_version: u32,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
    pub raw_delete_after: Option<DateTime<Utc>>,
    pub items: Vec<NewTalentImportItem>,
}

impl fmt::Debug for NewTalentImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewTalentImport")
            .field("batch_id", &self.batch_id)
            .field("source_type", &self.source_type)
            .field("source_name", &self.source_name)
            .field("imported_by", &self.imported_by)
            .field("raw_text_ciphertext", &"[REDACTED]")
            .field("encryption_key_version", &self.encryption_key_version)
            .field("model_provider", &self.model_provider)
            .field("model_name", &self.model_name)
            .field("prompt_version", &self.prompt_version)
            .field("raw_delete_after", &self.raw_delete_after)
            .field("items", &self.items)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct NewTalentImportItem {
    pub item_id: Uuid,
    pub position: u32,
    pub raw_fragment_ciphertext: Vec<u8>,
    pub encryption_key_version: u32,
    pub kind: TalentKind,
}

impl fmt::Debug for NewTalentImportItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewTalentImportItem")
            .field("item_id", &self.item_id)
            .field("position", &self.position)
            .field("raw_fragment_ciphertext", &"[REDACTED]")
            .field("encryption_key_version", &self.encryption_key_version)
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TalentImportBatchSummary {
    pub batch_id: Uuid,
    pub source_id: Uuid,
    pub source_type: SourceType,
    pub source_name: String,
    pub source_trust_status: SourceTrust,
    pub status: String,
    pub item_count: u32,
    pub processed_count: u32,
    pub failed_count: u32,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ClaimedTalentItem {
    pub item_id: Uuid,
    pub batch_id: Uuid,
    pub position: u32,
    pub raw_fragment_ciphertext: Vec<u8>,
    pub encryption_key_version: u32,
    pub kind: TalentKind,
    pub status: ImportItemStatus,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub claim_expires_at: DateTime<Utc>,
}

impl fmt::Debug for ClaimedTalentItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaimedTalentItem")
            .field("item_id", &self.item_id)
            .field("batch_id", &self.batch_id)
            .field("position", &self.position)
            .field("raw_fragment_ciphertext", &"[REDACTED]")
            .field("encryption_key_version", &self.encryption_key_version)
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("claim_expires_at", &self.claim_expires_at)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct SafeTalentExtraction(Value);

impl TryFrom<TalentExtraction> for SafeTalentExtraction {
    type Error = anyhow::Error;

    fn try_from(extraction: TalentExtraction) -> Result<Self> {
        if !extraction.contacts.is_empty() {
            bail!("safe talent extraction cannot contain contacts");
        }
        if safe_extraction_contains_private_data(&extraction) {
            bail!("safe talent extraction cannot contain protected strings");
        }
        let mut value = serde_json::to_value(extraction)?;
        value
            .as_object_mut()
            .expect("TalentExtraction serializes as an object")
            .remove("contacts");
        Ok(Self(value))
    }
}

impl Serialize for SafeTalentExtraction {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl fmt::Debug for SafeTalentExtraction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED SAFE JSON]")
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct CompletedTalentItem {
    pub item_id: Uuid,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub extraction: SafeTalentExtraction,
    pub extraction_confidence: f32,
    pub contact_candidates_ciphertext: Option<Vec<u8>>,
    pub contact_hashes: Vec<String>,
    pub contact_masked: Vec<PublicTalentContact>,
    pub normalized_fingerprint: Option<String>,
    pub validation_errors: Value,
    pub risk_score: f32,
    pub risk_level: RiskLevel,
    pub risk_reasons: Value,
    pub duplicate_candidates: Vec<DuplicateCandidate>,
}

impl fmt::Debug for CompletedTalentItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedTalentItem")
            .field("item_id", &self.item_id)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("extraction", &"[REDACTED SAFE JSON]")
            .field("extraction_confidence", &self.extraction_confidence)
            .field("contact_candidates_ciphertext", &"[REDACTED]")
            .field("contact_hashes", &"[REDACTED]")
            .field("contact_masked", &"[REDACTED SAFE JSON]")
            .field("normalized_fingerprint", &"[REDACTED]")
            .field("validation_errors", &"[REDACTED SAFE JSON]")
            .field("risk_score", &self.risk_score)
            .field("risk_level", &self.risk_level)
            .field("risk_reasons", &"[REDACTED SAFE JSON]")
            .field("duplicate_candidates", &"[REDACTED SAFE JSON]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct FailedTalentItem {
    pub item_id: Uuid,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub validation_errors: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TalentReviewFilter {
    pub statuses: Vec<ImportItemStatus>,
    pub kinds: Vec<TalentKind>,
    pub risk_levels: Vec<RiskLevel>,
    pub source_id: Option<Uuid>,
    pub updated_after: Option<DateTime<Utc>>,
    pub batch_id: Option<Uuid>,
    pub limit: u16,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TalentReviewItemSummary {
    pub item_id: Uuid,
    pub batch_id: Uuid,
    pub position: u32,
    pub kind: TalentKind,
    pub status: ImportItemStatus,
    pub extraction: Option<Value>,
    pub extraction_confidence: Option<f32>,
    pub contact_masked: Option<Value>,
    pub has_contact: bool,
    pub validation_errors: Option<Value>,
    pub risk_score: Option<f32>,
    pub risk_level: Option<RiskLevel>,
    pub risk_reasons: Option<Value>,
    pub duplicate_candidates: Option<Value>,
    pub lock_version: u32,
    pub processed_at: Option<DateTime<Utc>>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub source_id: Uuid,
    pub source_type: SourceType,
    pub source_name: String,
    pub source_trust_status: SourceTrust,
}

impl fmt::Debug for TalentReviewItemSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TalentReviewItemSummary")
            .field("item_id", &self.item_id)
            .field("batch_id", &self.batch_id)
            .field("position", &self.position)
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("extraction", &"[REDACTED SAFE JSON]")
            .field("extraction_confidence", &self.extraction_confidence)
            .field("contact_masked", &"[REDACTED SAFE JSON]")
            .field("has_contact", &self.has_contact)
            .field("validation_errors", &"[REDACTED SAFE JSON]")
            .field("risk_score", &self.risk_score)
            .field("risk_level", &self.risk_level)
            .field("risk_reasons", &"[REDACTED SAFE JSON]")
            .field("duplicate_candidates", &"[REDACTED SAFE JSON]")
            .field("lock_version", &self.lock_version)
            .field("processed_at", &self.processed_at)
            .field("reviewed_at", &self.reviewed_at)
            .field("updated_at", &self.updated_at)
            .field("source_id", &self.source_id)
            .field("source_type", &self.source_type)
            .field("source_name", &self.source_name)
            .field("source_trust_status", &self.source_trust_status)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct TalentReviewItem {
    pub item_id: Uuid,
    pub batch_id: Uuid,
    pub position: u32,
    pub raw_fragment_ciphertext: Option<Vec<u8>>,
    pub encryption_key_version: Option<u32>,
    pub contact_candidates_ciphertext: Option<Vec<u8>>,
    pub contact_hashes: Vec<String>,
    pub contact_masked: Option<Value>,
    pub normalized_fingerprint: Option<String>,
    pub kind: TalentKind,
    pub status: ImportItemStatus,
    pub extraction: Option<Value>,
    pub extraction_confidence: Option<f32>,
    pub validation_errors: Option<Value>,
    pub risk_score: Option<f32>,
    pub risk_level: Option<RiskLevel>,
    pub risk_reasons: Option<Value>,
    pub duplicate_candidates: Option<Value>,
    pub published_record_id: Option<Uuid>,
    pub lock_version: u32,
    pub processed_at: Option<DateTime<Utc>>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
    pub source_id: Uuid,
    pub source_type: SourceType,
    pub source_name: String,
    pub source_trust_status: SourceTrust,
    pub source_auto_publish_enabled: bool,
    pub source_confirmed_at: Option<DateTime<Utc>>,
    pub source_updated_at: DateTime<Utc>,
}

impl fmt::Debug for TalentReviewItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TalentReviewItem")
            .field("item_id", &self.item_id)
            .field("batch_id", &self.batch_id)
            .field("position", &self.position)
            .field("raw_fragment_ciphertext", &"[REDACTED]")
            .field("encryption_key_version", &self.encryption_key_version)
            .field("contact_candidates_ciphertext", &"[REDACTED]")
            .field("contact_hashes", &"[REDACTED]")
            .field("contact_masked", &"[REDACTED SAFE JSON]")
            .field("normalized_fingerprint", &"[REDACTED]")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("extraction", &"[REDACTED SAFE JSON]")
            .field("extraction_confidence", &self.extraction_confidence)
            .field("validation_errors", &"[REDACTED SAFE JSON]")
            .field("risk_score", &self.risk_score)
            .field("risk_level", &self.risk_level)
            .field("risk_reasons", &"[REDACTED SAFE JSON]")
            .field("duplicate_candidates", &"[REDACTED SAFE JSON]")
            .field("published_record_id", &self.published_record_id)
            .field("lock_version", &self.lock_version)
            .field("source_id", &self.source_id)
            .field("source_type", &self.source_type)
            .field("source_name", &self.source_name)
            .field("source_trust_status", &self.source_trust_status)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TalentDuplicateCandidate {
    pub candidate_item_id: Option<Uuid>,
    pub candidate_record_id: Option<Uuid>,
    pub kind: TalentKind,
    pub status: String,
    pub title: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub district: Option<String>,
    #[serde(default)]
    pub salary_min: Option<u32>,
    #[serde(default)]
    pub salary_max: Option<u32>,
    #[serde(default)]
    pub source_id: Option<Uuid>,
    #[serde(default)]
    pub normalized_text: String,
    pub score: f32,
    pub reasons: Vec<String>,
    pub high_similarity: bool,
}

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct TalentDuplicateProbe {
    pub item_id: Uuid,
    pub kind: TalentKind,
    pub title: Option<String>,
    pub roles: Vec<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub source_id: Uuid,
    pub contact_hashes: Vec<String>,
    pub normalized_fingerprint: Option<String>,
    pub normalized_text: String,
}

impl fmt::Debug for TalentDuplicateProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TalentDuplicateProbe")
            .field("item_id", &self.item_id)
            .field("kind", &self.kind)
            .field("title", &"[REDACTED SAFE TITLE]")
            .field("roles", &"[REDACTED SAFE ROLES]")
            .field("city", &"[REDACTED SAFE LOCATION]")
            .field("district", &"[REDACTED SAFE LOCATION]")
            .field("salary_min", &self.salary_min)
            .field("salary_max", &self.salary_max)
            .field("source_id", &self.source_id)
            .field("contact_hash_count", &self.contact_hashes.len())
            .field("normalized_fingerprint", &"[REDACTED]")
            .field("normalized_text", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for TalentDuplicateCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TalentDuplicateCandidate")
            .field("candidate_item_id", &self.candidate_item_id)
            .field("candidate_record_id", &self.candidate_record_id)
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("title", &"[REDACTED SAFE TITLE]")
            .field("roles", &"[REDACTED SAFE ROLES]")
            .field("city", &"[REDACTED SAFE LOCATION]")
            .field("district", &"[REDACTED SAFE LOCATION]")
            .field("salary_min", &self.salary_min)
            .field("salary_max", &self.salary_max)
            .field("source_id", &self.source_id)
            .field("normalized_text", &"[REDACTED]")
            .field("score", &self.score)
            .field("reasons", &self.reasons)
            .field("high_similarity", &self.high_similarity)
            .finish()
    }
}

impl From<TalentDuplicateCandidate> for DuplicateCandidate {
    fn from(candidate: TalentDuplicateCandidate) -> Self {
        Self {
            candidate_item_id: candidate.candidate_item_id,
            candidate_record_id: candidate.candidate_record_id,
            score: candidate.score,
            reasons: candidate.reasons,
            high_similarity: candidate.high_similarity,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct SaveTalentReview {
    pub item_id: Uuid,
    pub lock_version: u32,
    pub actor_user_id: Uuid,
    pub kind: Option<TalentKind>,
    pub extraction: Option<SafeTalentExtraction>,
    pub extraction_confidence: Option<f32>,
    pub contact_masked: Option<Vec<PublicTalentContact>>,
    pub validation_errors: Option<Value>,
    pub risk_score: Option<f32>,
    pub risk_level: Option<RiskLevel>,
    pub risk_reasons: Option<Value>,
    pub duplicate_candidates: Option<Vec<DuplicateCandidate>>,
    pub normalized_fingerprint: Option<String>,
    pub manual_policy_reasons: Option<Vec<ManualPolicyReason>>,
}

impl fmt::Debug for SaveTalentReview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveTalentReview")
            .field("item_id", &self.item_id)
            .field("lock_version", &self.lock_version)
            .field("actor_user_id", &self.actor_user_id)
            .field("kind", &self.kind)
            .field("extraction", &"[REDACTED SAFE JSON]")
            .field("extraction_confidence", &self.extraction_confidence)
            .field("contact_masked", &"[REDACTED SAFE JSON]")
            .field("validation_errors", &"[REDACTED SAFE JSON]")
            .field("risk_score", &self.risk_score)
            .field("risk_level", &self.risk_level)
            .field("risk_reasons", &"[REDACTED SAFE JSON]")
            .field("duplicate_candidates", &"[REDACTED SAFE JSON]")
            .field("normalized_fingerprint", &"[REDACTED]")
            .field("manual_policy_reasons", &self.manual_policy_reasons)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveReviewOutcome {
    Saved { lock_version: u32 },
    Conflict,
}

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct TalentRecordDraft {
    pub kind: TalentKind,
    pub title: String,
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
    pub source_label: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl fmt::Debug for TalentRecordDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TalentRecordDraft")
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("scene", &self.scene)
            .field("region", &self.region)
            .field("city", &self.city)
            .field("district", &self.district)
            .field("salary_min", &self.salary_min)
            .field("salary_max", &self.salary_max)
            .field("roles", &self.roles)
            .field("skills", &self.skills)
            .field("experience_years", &self.experience_years)
            .field("hours_per_day", &self.hours_per_day)
            .field("rest_days_per_month", &self.rest_days_per_month)
            .field("lodging", &self.lodging)
            .field("housing_allowance", &self.housing_allowance)
            .field("availability", &self.availability)
            .field("urgency", &self.urgency)
            .field("source_label", &self.source_label)
            .field("published_at", &self.published_at)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct MaskedContact(String);

impl MaskedContact {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for MaskedContact {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        let characters = value.chars().collect::<Vec<_>>();
        let first_mask = characters.iter().position(|character| *character == '*');
        let last_mask = characters.iter().rposition(|character| *character == '*');
        let valid = first_mask.zip(last_mask).is_some_and(|(first, last)| {
            let prefix = &characters[..first];
            let masks = &characters[first..=last];
            let suffix = &characters[last + 1..];
            characters.len() <= 64
                && prefix.len() <= 4
                && suffix.len() <= 4
                && masks.len() >= 3
                && masks.iter().all(|character| *character == '*')
                && prefix.iter().chain(suffix).all(|character| {
                    *character != '*' && !character.is_whitespace() && !character.is_control()
                })
        });
        if !valid {
            bail!("masked talent contact must contain masking");
        }
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for MaskedContact {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Self::try_from(value.as_str())?;
        Ok(Self(value))
    }
}

impl Serialize for MaskedContact {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MaskedContact {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Debug for MaskedContact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED MASKED CONTACT]")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuditReason {
    Duplicate,
    Rejected,
    Reprocess,
    Withdrawn,
    ModeratorCorrection,
    PolicyViolation,
    Expired,
    Other,
}

impl AuditReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Rejected => "rejected",
            Self::Reprocess => "reprocess",
            Self::Withdrawn => "withdrawn",
            Self::ModeratorCorrection => "moderator_correction",
            Self::PolicyViolation => "policy_violation",
            Self::Expired => "expired",
            Self::Other => "other",
        }
    }
}

impl TryFrom<&str> for AuditReason {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "duplicate" => Ok(Self::Duplicate),
            "rejected" => Ok(Self::Rejected),
            "reprocess" => Ok(Self::Reprocess),
            "withdrawn" => Ok(Self::Withdrawn),
            "moderator_correction" => Ok(Self::ModeratorCorrection),
            "policy_violation" => Ok(Self::PolicyViolation),
            "expired" => Ok(Self::Expired),
            "other" => Ok(Self::Other),
            _ => bail!("invalid talent audit reason"),
        }
    }
}

impl Serialize for AuditReason {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AuditReason {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value.as_str()).map_err(serde::de::Error::custom)
    }
}

impl fmt::Debug for AuditReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED SAFE AUDIT REASON]")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManualPolicyReason {
    GlobalAutoPublishPaused,
    SourceNotTrusted,
    SourceAutoPublishPaused,
    SourceNotConfirmed,
    RiskVersionMismatch,
    RiskNotLow,
    RiskHardBlock,
    UnsupportedKind,
    ConfidenceBelowThreshold,
    MissingRequiredFields,
    DuplicateHigh,
}

impl ManualPolicyReason {
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "global_auto_publish_paused" => Some(Self::GlobalAutoPublishPaused),
            "source_not_trusted" => Some(Self::SourceNotTrusted),
            "source_auto_publish_paused" => Some(Self::SourceAutoPublishPaused),
            "source_not_confirmed" => Some(Self::SourceNotConfirmed),
            "risk_version_mismatch" => Some(Self::RiskVersionMismatch),
            "risk_not_low" => Some(Self::RiskNotLow),
            "risk_hard_block" => Some(Self::RiskHardBlock),
            "unsupported_kind" => Some(Self::UnsupportedKind),
            "confidence_below_threshold" => Some(Self::ConfidenceBelowThreshold),
            "missing_required_fields" => Some(Self::MissingRequiredFields),
            "duplicate_high" => Some(Self::DuplicateHigh),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncryptedTalentContact {
    pub contact_type: ContactType,
    pub ciphertext: Vec<u8>,
    pub encryption_key_version: u32,
    pub normalized_hash: Option<String>,
    pub masked_value: MaskedContact,
}

impl fmt::Debug for EncryptedTalentContact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedTalentContact")
            .field("contact_type", &self.contact_type)
            .field("ciphertext", &"[REDACTED]")
            .field("encryption_key_version", &self.encryption_key_version)
            .field("normalized_hash", &"[REDACTED]")
            .field("masked_value", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct PublishTalentItem {
    pub item_id: Uuid,
    pub lock_version: u32,
    pub actor_user_id: Option<Uuid>,
    pub automatic: bool,
    pub record: TalentRecordDraft,
    pub contacts: Vec<EncryptedTalentContact>,
}

impl fmt::Debug for PublishTalentItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishTalentItem")
            .field("item_id", &self.item_id)
            .field("lock_version", &self.lock_version)
            .field("actor_user_id", &self.actor_user_id)
            .field("automatic", &self.automatic)
            .field("record", &"[REDACTED SAFE RECORD]")
            .field("contacts", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PublishOutcome {
    pub record_id: Uuid,
    pub created: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct MergeTalentImportItem {
    pub item_id: Uuid,
    pub lock_version: u32,
    pub target_record_id: Uuid,
    pub actor_user_id: Uuid,
    pub reason: Option<AuditReason>,
}

impl fmt::Debug for MergeTalentImportItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MergeTalentImportItem")
            .field("item_id", &self.item_id)
            .field("lock_version", &self.lock_version)
            .field("target_record_id", &self.target_record_id)
            .field("actor_user_id", &self.actor_user_id)
            .field("reason", &"[REDACTED SAFE REASON]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TransitionTalentImportItem {
    pub item_id: Uuid,
    pub lock_version: u32,
    pub next_status: ImportItemStatus,
    pub actor_user_id: Uuid,
    pub reason: Option<AuditReason>,
}

impl fmt::Debug for TransitionTalentImportItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransitionTalentImportItem")
            .field("item_id", &self.item_id)
            .field("lock_version", &self.lock_version)
            .field("next_status", &self.next_status)
            .field("actor_user_id", &self.actor_user_id)
            .field("reason", &"[REDACTED SAFE REASON]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PublicTalentContact {
    pub contact_type: ContactType,
    pub masked_value: MaskedContact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PublicTalentRecord {
    pub record_id: Uuid,
    pub kind: TalentKind,
    pub title: String,
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
    pub source_label: Option<String>,
    pub published_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub contact_masked: Vec<PublicTalentContact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TalentRuntimeSettings {
    pub auto_publish_enabled: bool,
    pub updated_by: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TalentSourceStatus {
    pub source_id: Uuid,
    pub trust_status: SourceTrust,
    pub auto_publish_enabled: bool,
    pub confirmed_by: Option<Uuid>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "resource", rename_all = "snake_case")]
pub(crate) enum ResourceUpdateOutcome<T> {
    Updated(T),
    Conflict,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TalentSourceSubscription {
    pub subscription_id: Uuid,
    pub source_id: Uuid,
    pub source_name: String,
    pub public_url: String,
    pub keywords: Vec<String>,
    pub interval_minutes: u32,
    pub enabled: bool,
    pub next_run_at: DateTime<Utc>,
    pub incremental_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub consecutive_failures: u32,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UpsertTalentSourceSubscription {
    pub subscription_id: Option<Uuid>,
    #[serde(skip_serializing)]
    pub source_name: String,
    pub public_url: String,
    pub keywords: Vec<String>,
    pub interval_minutes: u32,
    pub enabled: bool,
    pub actor_user_id: Uuid,
}

impl fmt::Debug for UpsertTalentSourceSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpsertTalentSourceSubscription")
            .field("subscription_id", &self.subscription_id)
            .field("source_name", &self.source_name)
            .field("public_url", &self.public_url)
            .field("keyword_count", &self.keywords.len())
            .field("interval_minutes", &self.interval_minutes)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ClaimedTalentSourceSubscription {
    pub run_id: Uuid,
    pub subscription: TalentSourceSubscription,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompleteTalentSourceRun {
    pub run_id: Uuid,
    pub discovered_count: u32,
    pub accepted_count: u32,
    pub duplicate_count: u32,
    pub failed_count: u32,
    pub error_code: Option<String>,
    pub incremental_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TalentCleanupOutcome {
    pub batches_cleaned: u64,
    pub items_cleaned: u64,
    pub cleaned_at: DateTime<Utc>,
}

impl fmt::Debug for FailedTalentItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FailedTalentItem")
            .field("item_id", &self.item_id)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("validation_errors", &"[REDACTED SAFE JSON]")
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct NewTalentImportWire<'a> {
    batch_id: Uuid,
    imported_by: Option<Uuid>,
    raw_text_ciphertext: String,
    encryption_key_version: u32,
    model_provider: &'a Option<String>,
    model_name: &'a Option<String>,
    prompt_version: &'a Option<String>,
    raw_delete_after: Option<DateTime<Utc>>,
    items: Vec<NewTalentImportItemWire>,
}

impl<'a> TryFrom<&'a NewTalentImport> for NewTalentImportWire<'a> {
    type Error = anyhow::Error;

    fn try_from(import: &'a NewTalentImport) -> Result<Self> {
        if !(1..=MAX_TALENT_IMPORT_ITEMS).contains(&import.items.len()) {
            bail!("talent import must contain between 1 and 500 items");
        }
        Ok(Self {
            batch_id: import.batch_id,
            imported_by: import.imported_by,
            raw_text_ciphertext: hex_encode(&import.raw_text_ciphertext),
            encryption_key_version: import.encryption_key_version,
            model_provider: &import.model_provider,
            model_name: &import.model_name,
            prompt_version: &import.prompt_version,
            raw_delete_after: import.raw_delete_after,
            items: import.items.iter().map(NewTalentImportItemWire::from).collect(),
        })
    }
}

#[derive(Debug, Serialize)]
struct NewTalentImportItemWire {
    item_id: Uuid,
    position: u32,
    raw_fragment_ciphertext: String,
    encryption_key_version: u32,
    kind: TalentKind,
}

impl From<&NewTalentImportItem> for NewTalentImportItemWire {
    fn from(item: &NewTalentImportItem) -> Self {
        Self {
            item_id: item.item_id,
            position: item.position,
            raw_fragment_ciphertext: hex_encode(&item.raw_fragment_ciphertext),
            encryption_key_version: item.encryption_key_version,
            kind: item.kind,
        }
    }
}

#[derive(Deserialize)]
struct ClaimedTalentItemWire {
    item_id: Uuid,
    batch_id: Uuid,
    position: u32,
    raw_fragment_ciphertext: String,
    encryption_key_version: u32,
    kind: TalentKind,
    status: ImportItemStatus,
    claim_token: Uuid,
    claim_version: i64,
    claim_expires_at: DateTime<Utc>,
}

impl ClaimedTalentItemWire {
    fn try_into_claimed(self) -> Result<ClaimedTalentItem> {
        if self.status != ImportItemStatus::Extracting {
            bail!("claimed talent import item is not extracting");
        }
        Ok(ClaimedTalentItem {
            item_id: self.item_id,
            batch_id: self.batch_id,
            position: self.position,
            raw_fragment_ciphertext: hex_decode(&self.raw_fragment_ciphertext)?,
            encryption_key_version: self.encryption_key_version,
            kind: self.kind,
            status: self.status,
            claim_token: self.claim_token,
            claim_version: self.claim_version,
            claim_expires_at: self.claim_expires_at,
        })
    }
}

#[derive(Debug, Serialize)]
struct CompletedTalentItemWire<'a> {
    item_id: Uuid,
    claim_token: Uuid,
    claim_version: i64,
    extraction: &'a SafeTalentExtraction,
    extraction_confidence: f32,
    contact_candidates_ciphertext: Option<String>,
    contact_hashes: &'a [String],
    contact_masked: &'a [PublicTalentContact],
    normalized_fingerprint: &'a Option<String>,
    validation_errors: &'a Value,
    risk_score: f32,
    risk_level: RiskLevel,
    risk_reasons: &'a Value,
    duplicate_candidates: &'a [DuplicateCandidate],
}

impl<'a> From<&'a CompletedTalentItem> for CompletedTalentItemWire<'a> {
    fn from(item: &'a CompletedTalentItem) -> Self {
        Self {
            item_id: item.item_id,
            claim_token: item.claim_token,
            claim_version: item.claim_version,
            extraction: &item.extraction,
            extraction_confidence: item.extraction_confidence,
            contact_candidates_ciphertext: item.contact_candidates_ciphertext.as_deref().map(hex_encode),
            contact_hashes: &item.contact_hashes,
            contact_masked: &item.contact_masked,
            normalized_fingerprint: &item.normalized_fingerprint,
            validation_errors: &item.validation_errors,
            risk_score: item.risk_score,
            risk_level: item.risk_level,
            risk_reasons: &item.risk_reasons,
            duplicate_candidates: &item.duplicate_candidates,
        }
    }
}

#[derive(Deserialize)]
struct TalentReviewItemWire {
    item_id: Uuid,
    batch_id: Uuid,
    position: u32,
    raw_fragment_ciphertext: Option<String>,
    encryption_key_version: Option<u32>,
    contact_candidates_ciphertext: Option<String>,
    #[serde(default)]
    contact_hashes: Vec<String>,
    contact_masked: Option<Value>,
    normalized_fingerprint: Option<String>,
    kind: TalentKind,
    status: ImportItemStatus,
    extraction: Option<Value>,
    extraction_confidence: Option<f32>,
    validation_errors: Option<Value>,
    risk_score: Option<f32>,
    risk_level: Option<RiskLevel>,
    risk_reasons: Option<Value>,
    duplicate_candidates: Option<Value>,
    published_record_id: Option<Uuid>,
    lock_version: u32,
    processed_at: Option<DateTime<Utc>>,
    reviewed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    model_provider: Option<String>,
    model_name: Option<String>,
    prompt_version: Option<String>,
    source_id: Uuid,
    source_type: SourceType,
    source_name: String,
    source_trust_status: SourceTrust,
    source_auto_publish_enabled: bool,
    source_confirmed_at: Option<DateTime<Utc>>,
    source_updated_at: DateTime<Utc>,
}

impl TalentReviewItemWire {
    fn try_into_review_item(self) -> Result<TalentReviewItem> {
        Ok(TalentReviewItem {
            item_id: self.item_id,
            batch_id: self.batch_id,
            position: self.position,
            raw_fragment_ciphertext: self.raw_fragment_ciphertext.as_deref().map(hex_decode).transpose()?,
            encryption_key_version: self.encryption_key_version,
            contact_candidates_ciphertext: self
                .contact_candidates_ciphertext
                .as_deref()
                .map(hex_decode)
                .transpose()?,
            contact_hashes: self.contact_hashes,
            contact_masked: self.contact_masked,
            normalized_fingerprint: self.normalized_fingerprint,
            kind: self.kind,
            status: self.status,
            extraction: self.extraction,
            extraction_confidence: self.extraction_confidence,
            validation_errors: self.validation_errors,
            risk_score: self.risk_score,
            risk_level: self.risk_level,
            risk_reasons: self.risk_reasons,
            duplicate_candidates: self.duplicate_candidates,
            published_record_id: self.published_record_id,
            lock_version: self.lock_version,
            processed_at: self.processed_at,
            reviewed_at: self.reviewed_at,
            updated_at: self.updated_at,
            model_provider: self.model_provider,
            model_name: self.model_name,
            prompt_version: self.prompt_version,
            source_id: self.source_id,
            source_type: self.source_type,
            source_name: self.source_name,
            source_trust_status: self.source_trust_status,
            source_auto_publish_enabled: self.source_auto_publish_enabled,
            source_confirmed_at: self.source_confirmed_at,
            source_updated_at: self.source_updated_at,
        })
    }
}

#[derive(Debug, Serialize)]
struct SaveTalentReviewWire<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<TalentKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction: Option<&'a SafeTalentExtraction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contact_masked: Option<&'a Vec<PublicTalentContact>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_errors: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk_level: Option<RiskLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk_reasons: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duplicate_candidates: Option<&'a Vec<DuplicateCandidate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_fingerprint: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manual_policy_reasons: Option<&'a Vec<ManualPolicyReason>>,
}

impl<'a> From<&'a SaveTalentReview> for SaveTalentReviewWire<'a> {
    fn from(review: &'a SaveTalentReview) -> Self {
        Self {
            kind: review.kind,
            extraction: review.extraction.as_ref(),
            extraction_confidence: review.extraction_confidence,
            contact_masked: review.contact_masked.as_ref(),
            validation_errors: review.validation_errors.as_ref(),
            risk_score: review.risk_score,
            risk_level: review.risk_level,
            risk_reasons: review.risk_reasons.as_ref(),
            duplicate_candidates: review.duplicate_candidates.as_ref(),
            normalized_fingerprint: review.normalized_fingerprint.as_ref(),
            manual_policy_reasons: review.manual_policy_reasons.as_ref(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EncryptedTalentContactWire<'a> {
    contact_type: ContactType,
    ciphertext: String,
    encryption_key_version: u32,
    normalized_hash: &'a Option<String>,
    masked_value: &'a str,
}

impl<'a> From<&'a EncryptedTalentContact> for EncryptedTalentContactWire<'a> {
    fn from(contact: &'a EncryptedTalentContact) -> Self {
        Self {
            contact_type: contact.contact_type,
            ciphertext: hex_encode(&contact.ciphertext),
            encryption_key_version: contact.encryption_key_version,
            normalized_hash: &contact.normalized_hash,
            masked_value: contact.masked_value.as_str(),
        }
    }
}

#[async_trait]
pub(crate) trait DBTalent {
    async fn create_talent_import(&self, talent_import: NewTalentImport) -> Result<Uuid>;
    async fn list_talent_import_batches(&self, limit: u16) -> Result<Vec<TalentImportBatchSummary>>;
    async fn claim_talent_import_item(&self) -> Result<Option<ClaimedTalentItem>>;
    async fn complete_talent_import_item(&self, item: CompletedTalentItem) -> Result<()>;
    async fn fail_talent_import_item(&self, item: FailedTalentItem) -> Result<()>;
    async fn list_talent_review_items(
        &self,
        filter: TalentReviewFilter,
    ) -> Result<Vec<TalentReviewItemSummary>>;
    async fn get_talent_review_item(&self, item_id: Uuid) -> Result<Option<TalentReviewItem>>;
    async fn find_talent_duplicate_candidates(
        &self,
        probe: TalentDuplicateProbe,
        limit: u16,
    ) -> Result<Vec<TalentDuplicateCandidate>>;
    async fn save_talent_review(&self, review: SaveTalentReview) -> Result<SaveReviewOutcome>;
    async fn publish_talent_item(&self, publication: PublishTalentItem) -> Result<PublishOutcome>;
    async fn merge_talent_import_item(&self, merge: MergeTalentImportItem) -> Result<SaveReviewOutcome>;
    async fn transition_talent_import_item(
        &self,
        transition: TransitionTalentImportItem,
    ) -> Result<SaveReviewOutcome>;
    async fn withdraw_talent_record(
        &self,
        item_id: Uuid,
        record_id: Uuid,
        lock_version: u32,
        actor_user_id: Uuid,
        reason: Option<AuditReason>,
    ) -> Result<SaveReviewOutcome>;
    async fn list_published_talent_records(
        &self,
        kind: Option<TalentKind>,
        limit: u16,
    ) -> Result<Vec<PublicTalentRecord>>;
    async fn get_talent_runtime_settings(&self) -> Result<TalentRuntimeSettings>;
    async fn set_talent_auto_publish(
        &self,
        enabled: bool,
        actor_user_id: Uuid,
        expected_updated_at: DateTime<Utc>,
    ) -> Result<ResourceUpdateOutcome<TalentRuntimeSettings>>;
    async fn set_talent_source_status(
        &self,
        source_id: Uuid,
        trust_status: SourceTrust,
        actor_user_id: Uuid,
        auto_publish_enabled: bool,
        expected_updated_at: DateTime<Utc>,
    ) -> Result<ResourceUpdateOutcome<TalentSourceStatus>>;
    async fn cleanup_talent_raw_data(&self, now: DateTime<Utc>) -> Result<TalentCleanupOutcome>;
    async fn upsert_talent_source_subscription(
        &self,
        subscription: UpsertTalentSourceSubscription,
    ) -> Result<Uuid>;
    async fn list_talent_source_subscriptions(&self) -> Result<Vec<TalentSourceSubscription>>;
    async fn claim_talent_source_subscription(&self) -> Result<Option<ClaimedTalentSourceSubscription>>;
    async fn complete_talent_source_run(&self, run: CompleteTalentSourceRun) -> Result<bool>;
    async fn set_talent_source_subscription_enabled(
        &self,
        subscription_id: Uuid,
        enabled: bool,
        actor_user_id: Uuid,
    ) -> Result<bool>;
    async fn reserve_talent_source_item(
        &self,
        subscription_id: Uuid,
        source_item_id: &str,
        content_fingerprint: &str,
    ) -> Result<bool>;
    async fn release_talent_source_item(
        &self,
        subscription_id: Uuid,
        source_item_id: &str,
        content_fingerprint: &str,
    ) -> Result<bool>;
}

#[async_trait]
impl DBTalent for PgDB {
    #[instrument(skip(self, talent_import), err)]
    async fn create_talent_import(&self, talent_import: NewTalentImport) -> Result<Uuid> {
        trace!("db: create encrypted talent import");
        let wire = NewTalentImportWire::try_from(&talent_import)?;
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                with source as (
                    select upsert_talent_source($1::text, $2::text, $3::uuid) as source_id
                )
                select create_talent_import_batch(
                    $4::jsonb || jsonb_build_object('source_id', source.source_id)
                )
                from source;
                ",
                &[
                    &source_type_name(talent_import.source_type),
                    &talent_import.source_name,
                    &talent_import.imported_by,
                    &Json(&wire),
                ],
            )
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self), err)]
    async fn list_talent_import_batches(&self, limit: u16) -> Result<Vec<TalentImportBatchSummary>> {
        trace!("db: list talent import batches");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select list_talent_import_batches($1::integer)::text;",
                &[&i32::from(limit)],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn claim_talent_import_item(&self) -> Result<Option<ClaimedTalentItem>> {
        trace!("db: claim encrypted talent import item");
        let db = self.pool.get().await?;
        let row = db.query_one("select claim_talent_import_item()::text;", &[]).await?;
        let claimed = row
            .get::<_, Option<String>>(0)
            .map(|json| serde_json::from_str::<ClaimedTalentItemWire>(&json))
            .transpose()?
            .map(ClaimedTalentItemWire::try_into_claimed)
            .transpose()?;
        Ok(claimed)
    }

    #[instrument(skip(self, item), err)]
    async fn complete_talent_import_item(&self, item: CompletedTalentItem) -> Result<()> {
        trace!("db: complete encrypted talent import item");
        let wire = CompletedTalentItemWire::from(&item);
        let db = self.pool.get().await?;
        db.execute("select complete_talent_import_item($1::jsonb);", &[&Json(&wire)])
            .await?;
        Ok(())
    }

    #[instrument(skip(self, item), err)]
    async fn fail_talent_import_item(&self, item: FailedTalentItem) -> Result<()> {
        trace!("db: fail encrypted talent import item");
        let db = self.pool.get().await?;
        db.execute("select fail_talent_import_item($1::jsonb);", &[&Json(&item)])
            .await?;
        Ok(())
    }

    #[instrument(skip(self, filter), err)]
    async fn list_talent_review_items(
        &self,
        filter: TalentReviewFilter,
    ) -> Result<Vec<TalentReviewItemSummary>> {
        trace!("db: list talent review items");
        let statuses = filter
            .statuses
            .iter()
            .copied()
            .map(import_item_status_name)
            .collect::<Vec<_>>();
        let kinds = filter.kinds.iter().copied().map(talent_kind_name).collect::<Vec<_>>();
        let risk_levels = filter
            .risk_levels
            .iter()
            .copied()
            .map(risk_level_name)
            .collect::<Vec<_>>();
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                select list_talent_review_items(
                    $1::text[], $2::uuid, $3::integer, $4::text[],
                    $5::text[], $6::timestamptz, $7::uuid
                )::text;
                ",
                &[
                    &statuses,
                    &filter.source_id,
                    &i32::from(filter.limit),
                    &kinds,
                    &risk_levels,
                    &filter.updated_after,
                    &filter.batch_id,
                ],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn get_talent_review_item(&self, item_id: Uuid) -> Result<Option<TalentReviewItem>> {
        trace!("db: get encrypted talent review item");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select get_talent_review_item($1::uuid)::text;", &[&item_id])
            .await?;
        row.get::<_, Option<String>>(0)
            .map(|json| serde_json::from_str::<TalentReviewItemWire>(&json))
            .transpose()?
            .map(TalentReviewItemWire::try_into_review_item)
            .transpose()
    }

    #[instrument(skip(self, probe), err)]
    async fn find_talent_duplicate_candidates(
        &self,
        probe: TalentDuplicateProbe,
        limit: u16,
    ) -> Result<Vec<TalentDuplicateCandidate>> {
        trace!("db: find talent duplicate candidates");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select find_talent_duplicate_candidates($1::jsonb, $2::integer)::text;",
                &[&Json(&probe), &i32::from(limit)],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self, review), err)]
    async fn save_talent_review(&self, review: SaveTalentReview) -> Result<SaveReviewOutcome> {
        trace!("db: save talent review");
        let updates = SaveTalentReviewWire::from(&review);
        let db = self.pool.get().await?;
        let row = db
            .query_opt(
                "
                select lock_version
                from update_talent_review_item($1::uuid, $2::integer, $3::jsonb, $4::uuid);
                ",
                &[
                    &review.item_id,
                    &i32::try_from(review.lock_version)?,
                    &Json(&updates),
                    &review.actor_user_id,
                ],
            )
            .await?;
        Ok(match row {
            Some(row) => SaveReviewOutcome::Saved {
                lock_version: row.get::<_, i32>(0).try_into()?,
            },
            None => SaveReviewOutcome::Conflict,
        })
    }

    #[instrument(skip(self, publication), err)]
    async fn publish_talent_item(&self, publication: PublishTalentItem) -> Result<PublishOutcome> {
        trace!("db: publish talent item");
        let contacts = publication
            .contacts
            .iter()
            .map(EncryptedTalentContactWire::from)
            .collect::<Vec<_>>();
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                select record_id, created
                from publish_talent_item(
                    $1::uuid, $2::integer, $3::uuid, $4::boolean, $5::jsonb, $6::jsonb
                );
                ",
                &[
                    &publication.item_id,
                    &i32::try_from(publication.lock_version)?,
                    &publication.actor_user_id,
                    &publication.automatic,
                    &Json(&publication.record),
                    &Json(&contacts),
                ],
            )
            .await?;
        Ok(PublishOutcome {
            record_id: row.get(0),
            created: row.get(1),
        })
    }

    #[instrument(skip(self, merge), err)]
    async fn merge_talent_import_item(&self, merge: MergeTalentImportItem) -> Result<SaveReviewOutcome> {
        trace!("db: merge talent import item");
        let reason = merge.reason.map(AuditReason::as_str);
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                select merge_talent_import_item(
                    $1::uuid, $2::integer, $3::uuid, $4::uuid, $5::text
                );
                ",
                &[
                    &merge.item_id,
                    &i32::try_from(merge.lock_version)?,
                    &merge.target_record_id,
                    &merge.actor_user_id,
                    &reason,
                ],
            )
            .await?;
        Ok(if row.get::<_, bool>(0) {
            SaveReviewOutcome::Saved {
                lock_version: merge.lock_version + 1,
            }
        } else {
            SaveReviewOutcome::Conflict
        })
    }

    #[instrument(skip(self, transition), err)]
    async fn transition_talent_import_item(
        &self,
        transition: TransitionTalentImportItem,
    ) -> Result<SaveReviewOutcome> {
        trace!("db: transition talent import item");
        let reason = transition.reason.map(AuditReason::as_str);
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                select transition_talent_import_item(
                    $1::uuid, $2::integer, $3::text, $4::uuid, $5::text
                );
                ",
                &[
                    &transition.item_id,
                    &i32::try_from(transition.lock_version)?,
                    &import_item_status_name(transition.next_status),
                    &transition.actor_user_id,
                    &reason,
                ],
            )
            .await?;
        Ok(if row.get::<_, bool>(0) {
            SaveReviewOutcome::Saved {
                lock_version: transition.lock_version + 1,
            }
        } else {
            SaveReviewOutcome::Conflict
        })
    }

    #[instrument(skip(self, reason), err)]
    async fn withdraw_talent_record(
        &self,
        item_id: Uuid,
        record_id: Uuid,
        lock_version: u32,
        actor_user_id: Uuid,
        reason: Option<AuditReason>,
    ) -> Result<SaveReviewOutcome> {
        trace!("db: withdraw talent record");
        let reason = reason.map(AuditReason::as_str);
        let db = self.pool.get().await?;
        let row = db
            .query_opt(
                "
                select withdraw_talent_record(
                    $1::uuid, $2::uuid, $3::integer, $4::uuid, $5::text
                );
                ",
                &[
                    &item_id,
                    &record_id,
                    &i32::try_from(lock_version)?,
                    &actor_user_id,
                    &reason,
                ],
            )
            .await?;
        Ok(match row.and_then(|row| row.get::<_, Option<i32>>(0)) {
            Some(next_version) => SaveReviewOutcome::Saved {
                lock_version: next_version.try_into()?,
            },
            None => SaveReviewOutcome::Conflict,
        })
    }

    #[instrument(skip(self), err)]
    async fn list_published_talent_records(
        &self,
        kind: Option<TalentKind>,
        limit: u16,
    ) -> Result<Vec<PublicTalentRecord>> {
        trace!("db: list public talent records");
        let kind = kind.map(talent_kind_name);
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select list_published_talent_records($1::text, $2::integer)::text;",
                &[&kind, &i32::from(limit)],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn get_talent_runtime_settings(&self) -> Result<TalentRuntimeSettings> {
        trace!("db: get talent runtime settings");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select get_talent_runtime_settings()::text;", &[])
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn set_talent_auto_publish(
        &self,
        enabled: bool,
        actor_user_id: Uuid,
        expected_updated_at: DateTime<Utc>,
    ) -> Result<ResourceUpdateOutcome<TalentRuntimeSettings>> {
        trace!("db: set talent auto-publish");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select set_talent_auto_publish($1::boolean, $2::uuid, $3::timestamptz)::text;",
                &[&enabled, &actor_user_id, &expected_updated_at],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn set_talent_source_status(
        &self,
        source_id: Uuid,
        trust_status: SourceTrust,
        actor_user_id: Uuid,
        auto_publish_enabled: bool,
        expected_updated_at: DateTime<Utc>,
    ) -> Result<ResourceUpdateOutcome<TalentSourceStatus>> {
        trace!("db: set talent source status");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "
                select set_talent_source_status(
                    $1::uuid, $2::text, $3::uuid, $4::boolean, $5::timestamptz
                )::text;
                ",
                &[
                    &source_id,
                    &source_trust_name(trust_status),
                    &actor_user_id,
                    &auto_publish_enabled,
                    &expected_updated_at,
                ],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn cleanup_talent_raw_data(&self, now: DateTime<Utc>) -> Result<TalentCleanupOutcome> {
        trace!("db: clean expired encrypted talent raw data");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select cleanup_talent_raw_data($1::timestamptz)::text;", &[&now])
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self, subscription), err)]
    async fn upsert_talent_source_subscription(
        &self,
        subscription: UpsertTalentSourceSubscription,
    ) -> Result<Uuid> {
        trace!("db: upsert talent source subscription");
        let payload = serde_json::to_value(&subscription)?;
        let db = self.pool.get().await?;
        let row = db.query_one(
            "with source as (select upsert_talent_source('web', $1::text, $2::uuid) source_id) select upsert_talent_source_subscription($3::jsonb || jsonb_build_object('source_id', source.source_id)) from source;",
            &[&subscription.source_name, &subscription.actor_user_id, &payload],
        ).await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self), err)]
    async fn list_talent_source_subscriptions(&self) -> Result<Vec<TalentSourceSubscription>> {
        trace!("db: list talent source subscriptions");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select list_talent_source_subscriptions()::text;", &[])
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn claim_talent_source_subscription(&self) -> Result<Option<ClaimedTalentSourceSubscription>> {
        trace!("db: claim talent source subscription");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select claim_talent_source_subscription()::text;", &[])
            .await?;
        row.get::<_, Option<String>>(0)
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }

    #[instrument(skip(self, run), err)]
    async fn complete_talent_source_run(&self, run: CompleteTalentSourceRun) -> Result<bool> {
        trace!("db: complete talent source run");
        let payload = serde_json::to_value(run)?;
        let db = self.pool.get().await?;
        let row = db
            .query_one("select complete_talent_source_run($1::jsonb);", &[&payload])
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self), err)]
    async fn set_talent_source_subscription_enabled(
        &self,
        subscription_id: Uuid,
        enabled: bool,
        actor_user_id: Uuid,
    ) -> Result<bool> {
        trace!("db: set talent source subscription enabled");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select set_talent_source_subscription_enabled($1::uuid, $2::boolean, $3::uuid);",
                &[&subscription_id, &enabled, &actor_user_id],
            )
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self, source_item_id, content_fingerprint), err)]
    async fn reserve_talent_source_item(
        &self,
        subscription_id: Uuid,
        source_item_id: &str,
        content_fingerprint: &str,
    ) -> Result<bool> {
        trace!("db: reserve talent source item");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select reserve_talent_source_item($1::uuid, $2::text, $3::text);",
                &[&subscription_id, &source_item_id, &content_fingerprint],
            )
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self, source_item_id, content_fingerprint), err)]
    async fn release_talent_source_item(
        &self,
        subscription_id: Uuid,
        source_item_id: &str,
        content_fingerprint: &str,
    ) -> Result<bool> {
        trace!("db: release talent source item");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select release_talent_source_item($1::uuid, $2::text, $3::text);",
                &[&subscription_id, &source_item_id, &content_fingerprint],
            )
            .await?;
        Ok(row.get(0))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

const fn source_type_name(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::WechatGroup => "wechat_group",
        SourceType::QqGroup => "qq_group",
        SourceType::Web => "web",
        SourceType::Other => "other",
    }
}

const fn source_trust_name(source_trust: SourceTrust) -> &'static str {
    match source_trust {
        SourceTrust::New => "new",
        SourceTrust::Trusted => "trusted",
        SourceTrust::Watchlist => "watchlist",
        SourceTrust::Blocked => "blocked",
    }
}

const fn talent_kind_name(kind: TalentKind) -> &'static str {
    match kind {
        TalentKind::Hiring => "hiring",
        TalentKind::Seeking => "seeking",
        TalentKind::Irrelevant => "irrelevant",
        TalentKind::Uncertain => "uncertain",
    }
}

const fn import_item_status_name(status: ImportItemStatus) -> &'static str {
    match status {
        ImportItemStatus::Queued => "queued",
        ImportItemStatus::Extracting => "extracting",
        ImportItemStatus::NeedsReview => "needs_review",
        ImportItemStatus::AutoPublished => "auto_published",
        ImportItemStatus::Published => "published",
        ImportItemStatus::Rejected => "rejected",
        ImportItemStatus::Merged => "merged",
        ImportItemStatus::Failed => "failed",
    }
}

const fn risk_level_name(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

fn hex_decode(encoded: &str) -> Result<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        bail!("encrypted talent import item has invalid encoding");
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("encrypted talent import item has invalid encoding"),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use crate::talent::{
        ContactCandidate, ContactType, DuplicateCandidate, ImportItemStatus, RiskLevel, TalentExtraction,
        TalentKind, mask_contact,
    };

    use super::{
        AuditReason, ClaimedTalentItemWire, EncryptedTalentContact, ManualPolicyReason, MaskedContact,
        NewTalentImport, NewTalentImportItem, NewTalentImportWire, PublicTalentContact, PublicTalentRecord,
        SafeTalentExtraction, SaveReviewOutcome, SaveTalentReview, SaveTalentReviewWire,
        TalentDuplicateCandidate,
    };

    fn new_import(items: Vec<NewTalentImportItem>) -> NewTalentImport {
        NewTalentImport {
            batch_id: Uuid::new_v4(),
            source_type: crate::talent::SourceType::Other,
            source_name: "Synthetic Source".to_owned(),
            imported_by: None,
            raw_text_ciphertext: vec![0, 1, 254, 255],
            encryption_key_version: 2,
            model_provider: Some("synthetic-provider".to_owned()),
            model_name: Some("synthetic-model".to_owned()),
            prompt_version: Some("talent-extract-v1".to_owned()),
            raw_delete_after: Some(Utc::now()),
            items,
        }
    }

    #[test]
    fn import_wire_contains_only_hex_encoded_ciphertext() {
        let input = new_import(vec![NewTalentImportItem {
            item_id: Uuid::new_v4(),
            position: 0,
            raw_fragment_ciphertext: vec![10, 11, 12],
            encryption_key_version: 2,
            kind: TalentKind::Hiring,
        }]);

        let wire = serde_json::to_value(NewTalentImportWire::try_from(&input).unwrap()).unwrap();

        assert!(wire.get("source_id").is_none());
        assert_eq!(wire["batch_id"], input.batch_id.to_string());
        assert_eq!(wire["items"][0]["item_id"], input.items[0].item_id.to_string());
        assert_eq!(wire["raw_text_ciphertext"], "0001feff");
        assert_eq!(wire["items"][0]["raw_fragment_ciphertext"], "0a0b0c");
        assert!(wire.to_string().find("[0,1,254,255]").is_none());
    }

    #[test]
    fn import_boundary_rejects_item_counts_outside_the_hard_limit() {
        for items in [
            vec![],
            (0..501)
                .map(|position| NewTalentImportItem {
                    item_id: Uuid::new_v4(),
                    position,
                    raw_fragment_ciphertext: vec![1],
                    encryption_key_version: 1,
                    kind: TalentKind::Hiring,
                })
                .collect(),
        ] {
            let Err(error) = NewTalentImportWire::try_from(&new_import(items)) else {
                panic!("out-of-range import should be rejected");
            };

            assert_eq!(
                error.to_string(),
                "talent import must contain between 1 and 500 items"
            );
        }
    }

    #[test]
    fn safe_extraction_rejects_complete_contacts_hidden_in_non_contact_fields() {
        let unsafe_extraction = TalentExtraction {
            title: Some("Call 13800138000".to_owned()),
            ..TalentExtraction::default()
        };

        let error = SafeTalentExtraction::try_from(unsafe_extraction).unwrap_err();

        assert_eq!(
            error.to_string(),
            "safe talent extraction cannot contain protected strings"
        );
    }

    #[test]
    fn safe_extraction_rejects_complete_contacts() {
        let unsafe_extraction = TalentExtraction {
            contacts: vec![ContactCandidate {
                contact_type: ContactType::Wechat,
                value: "fictional_complete_contact_001".to_owned(),
            }],
            ..TalentExtraction::default()
        };

        let error = SafeTalentExtraction::try_from(unsafe_extraction).unwrap_err();

        assert_eq!(
            error.to_string(),
            "safe talent extraction cannot contain contacts"
        );
    }

    #[test]
    fn safe_extraction_serializes_without_a_contacts_key() {
        let safe = SafeTalentExtraction::try_from(TalentExtraction::default()).unwrap();

        let serialized = serde_json::to_value(safe).unwrap();

        assert!(serialized.get("contacts").is_none());
    }

    #[test]
    fn claimed_item_wire_decodes_ciphertext_without_a_plaintext_field() {
        let item_id = Uuid::new_v4();
        let batch_id = Uuid::new_v4();
        let claim_token = Uuid::new_v4();
        let wire: ClaimedTalentItemWire = serde_json::from_value(json!({
            "item_id": item_id,
            "batch_id": batch_id,
            "position": 3,
            "raw_fragment_ciphertext": "0001feff",
            "encryption_key_version": 2,
            "kind": "hiring",
            "status": "extracting",
            "claim_token": claim_token,
            "claim_version": 7,
            "claim_expires_at": "2026-07-29T12:34:56Z"
        }))
        .unwrap();

        let claimed = wire.try_into_claimed().unwrap();

        assert_eq!(claimed.item_id, item_id);
        assert_eq!(claimed.batch_id, batch_id);
        assert_eq!(claimed.raw_fragment_ciphertext, [0, 1, 254, 255]);
        assert_eq!(claimed.status, ImportItemStatus::Extracting);
        assert_eq!(claimed.claim_token, claim_token);
        assert_eq!(claimed.claim_version, 7);
        assert_eq!(claimed.claim_expires_at.to_rfc3339(), "2026-07-29T12:34:56+00:00");
    }

    #[test]
    fn completion_types_accept_safe_json_and_encrypted_contacts_only() {
        let extraction = SafeTalentExtraction::try_from(TalentExtraction {
            kind: TalentKind::Hiring,
            title: Some("Synthetic role".to_owned()),
            ..TalentExtraction::default()
        })
        .unwrap();
        let completed = super::CompletedTalentItem {
            item_id: Uuid::new_v4(),
            claim_token: Uuid::new_v4(),
            claim_version: 7,
            extraction,
            extraction_confidence: 0.75,
            contact_candidates_ciphertext: Some(vec![1, 2, 3]),
            contact_hashes: vec!["synthetic-hash".to_owned()],
            contact_masked: vec![PublicTalentContact {
                contact_type: ContactType::Wechat,
                masked_value: MaskedContact::try_from("tes***_01").unwrap(),
            }],
            normalized_fingerprint: Some("synthetic-fingerprint".to_owned()),
            validation_errors: json!([{"message": "private-validation-detail"}]),
            risk_score: 10.0,
            risk_level: RiskLevel::Low,
            risk_reasons: json!([{"evidence": "private-risk-evidence"}]),
            duplicate_candidates: vec![DuplicateCandidate {
                candidate_item_id: Some(Uuid::new_v4()),
                candidate_record_id: None,
                score: 1.0,
                reasons: vec!["private-duplicate-detail".to_owned()],
                high_similarity: true,
            }],
        };

        let debug = format!("{completed:?}");
        assert!(!debug.contains("[1, 2, 3]"));
        assert!(!debug.contains("Synthetic role"));
        assert!(!debug.contains("synthetic-hash"));
        assert!(!debug.contains("syn***001"));
        assert!(!debug.contains("synthetic-fingerprint"));
        assert!(!debug.contains("private-validation-detail"));
        assert!(!debug.contains("private-risk-evidence"));
        assert!(!debug.contains("private-duplicate-detail"));
        assert!(debug.contains("[REDACTED]"));

        let wire = serde_json::to_value(super::CompletedTalentItemWire::from(&completed)).unwrap();
        assert_eq!(wire["contact_masked"][0]["masked_value"], "tes***_01");

        let failed = super::FailedTalentItem {
            item_id: Uuid::new_v4(),
            claim_token: Uuid::new_v4(),
            claim_version: 7,
            validation_errors: json!([{"message": "private-failure-detail"}]),
        };
        assert!(!format!("{failed:?}").contains("private-failure-detail"));
    }

    #[test]
    fn encrypted_publication_contact_debug_redacts_every_private_field() {
        let contact = EncryptedTalentContact {
            contact_type: ContactType::Wechat,
            ciphertext: vec![1, 2, 3],
            encryption_key_version: 1,
            normalized_hash: Some("fictional_complete_contact_901".to_owned()),
            masked_value: MaskedContact::try_from("fic***901").unwrap(),
        };

        let debug = format!("{contact:?}");

        assert!(!debug.contains("[1, 2, 3]"));
        assert!(!debug.contains("fictional_complete_contact_901"));
        assert!(!debug.contains("fic***901"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn public_talent_record_serialization_has_masked_contacts_only() {
        let record = PublicTalentRecord {
            record_id: Uuid::new_v4(),
            kind: TalentKind::Seeking,
            title: "Synthetic Candidate".to_owned(),
            scene: None,
            region: None,
            city: Some("Test City".to_owned()),
            district: None,
            salary_min: None,
            salary_max: None,
            roles: vec!["Rust".to_owned()],
            skills: vec![],
            experience_years: None,
            hours_per_day: None,
            rest_days_per_month: None,
            lodging: Some("\u{53ef}\u{534f}\u{5546}".to_owned()),
            housing_allowance: Some(900),
            availability: None,
            urgency: None,
            source_label: Some("Synthetic Source".to_owned()),
            published_at: Utc::now(),
            expires_at: None,
            contact_masked: vec![PublicTalentContact {
                contact_type: ContactType::Wechat,
                masked_value: MaskedContact::try_from("fic***902").unwrap(),
            }],
        };

        let value = serde_json::to_value(record).unwrap();
        let serialized = value.to_string();

        assert_eq!(value["contact_masked"][0]["masked_value"], "fic***902");
        assert_eq!(value["lodging"], "\u{53ef}\u{534f}\u{5546}");
        assert_eq!(value["housing_allowance"], 900);
        for forbidden in [
            "ciphertext",
            "normalized_hash",
            "raw_fragment",
            "contact_candidates",
            "fictional_complete_contact",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn stale_review_has_a_typed_conflict_outcome() {
        assert_eq!(SaveReviewOutcome::Conflict, SaveReviewOutcome::Conflict);
    }

    #[test]
    fn masked_contact_boundary_rejects_plausible_complete_contacts() {
        for unsafe_value in ["13800138000", "fictionalCompleteWechat903"] {
            let error = MaskedContact::try_from(unsafe_value).unwrap_err();
            assert_eq!(error.to_string(), "masked talent contact must contain masking");
            assert!(!error.to_string().contains(unsafe_value));

            let deserialize_error =
                serde_json::from_str::<MaskedContact>(&format!("\"{unsafe_value}\"")).unwrap_err();
            assert!(!deserialize_error.to_string().contains(unsafe_value));
        }

        for (contact_type, raw, expected) in [
            (ContactType::Wechat, "test_candidate_01", "tes***_01"),
            (ContactType::Phone, "+8613800000001", "+86****0001"),
        ] {
            let canonical = mask_contact(contact_type, raw);
            assert_eq!(canonical, expected);
            assert_eq!(MaskedContact::try_from(canonical).unwrap().as_str(), expected);
        }
    }

    #[test]
    fn audit_reason_boundary_accepts_codes_and_rejects_caller_text() {
        assert_eq!(
            AuditReason::try_from("withdrawn").unwrap(),
            AuditReason::Withdrawn
        );

        let unsafe_value = "fictional_complete_contact_904";
        let error = AuditReason::try_from(unsafe_value).unwrap_err();
        assert_eq!(error.to_string(), "invalid talent audit reason");
        assert!(!error.to_string().contains(unsafe_value));

        let deserialize_error =
            serde_json::from_str::<AuditReason>(&format!("\"{unsafe_value}\"")).unwrap_err();
        assert!(!deserialize_error.to_string().contains(unsafe_value));
    }

    #[test]
    fn partial_review_wire_omits_fields_that_were_not_updated() {
        let mut review = SaveTalentReview {
            item_id: Uuid::new_v4(),
            lock_version: 0,
            actor_user_id: Uuid::new_v4(),
            kind: None,
            extraction: None,
            extraction_confidence: None,
            contact_masked: None,
            validation_errors: None,
            risk_score: None,
            risk_level: None,
            risk_reasons: None,
            duplicate_candidates: None,
            normalized_fingerprint: None,
            manual_policy_reasons: None,
        };

        let value = serde_json::to_value(SaveTalentReviewWire::from(&review)).unwrap();

        assert_eq!(value, json!({}));

        review.manual_policy_reasons = Some(vec![ManualPolicyReason::SourceNotTrusted]);
        let value = serde_json::to_value(SaveTalentReviewWire::from(&review)).unwrap();

        assert_eq!(value, json!({"manual_policy_reasons": ["source_not_trusted"]}));
    }

    #[test]
    fn duplicate_lookup_candidate_maps_to_exact_persistence_schema() {
        let lookup = TalentDuplicateCandidate {
            candidate_item_id: Some(Uuid::new_v4()),
            candidate_record_id: None,
            kind: TalentKind::Hiring,
            status: "needs_review".to_owned(),
            title: Some("Synthetic duplicate title".to_owned()),
            roles: vec!["Synthetic Role".to_owned()],
            city: Some("Synthetic City".to_owned()),
            district: Some("Synthetic District".to_owned()),
            salary_min: Some(1_000),
            salary_max: Some(2_000),
            source_id: Some(Uuid::new_v4()),
            normalized_text: "synthetic duplicate title role city district".to_owned(),
            score: 1.0,
            reasons: vec!["exact_fingerprint".to_owned()],
            high_similarity: true,
        };

        let debug = format!("{lookup:?}");
        assert!(!debug.contains("Synthetic duplicate title"));
        assert!(!debug.contains("synthetic duplicate title role city district"));

        let persisted = DuplicateCandidate::from(lookup);
        let value = serde_json::to_value(persisted).unwrap();
        let fields = value.as_object().unwrap();

        assert_eq!(fields.len(), 5);
        assert!(fields.contains_key("candidate_item_id"));
        assert!(fields.contains_key("candidate_record_id"));
        assert!(fields.contains_key("score"));
        assert!(fields.contains_key("reasons"));
        assert!(fields.contains_key("high_similarity"));
        assert!(!fields.contains_key("kind"));
        assert!(!fields.contains_key("status"));
        assert!(!fields.contains_key("title"));
    }
}
