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
    talent::{ImportItemStatus, RiskLevel, SourceTrust, SourceType, TalentKind},
};

#[derive(Clone, PartialEq)]
pub(crate) struct NewTalentImport {
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
    pub position: u32,
    pub raw_fragment_ciphertext: Vec<u8>,
    pub encryption_key_version: u32,
    pub kind: TalentKind,
}

impl fmt::Debug for NewTalentImportItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewTalentImportItem")
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
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct CompletedTalentItem {
    pub item_id: Uuid,
    pub status: ImportItemStatus,
    /// Extraction JSON after raw contact candidates have been removed.
    pub extraction: Value,
    pub extraction_confidence: f32,
    pub contact_candidates_ciphertext: Option<Vec<u8>>,
    pub contact_hashes: Vec<String>,
    pub contact_masked: Value,
    pub normalized_fingerprint: Option<String>,
    pub validation_errors: Value,
    pub risk_score: f32,
    pub risk_level: RiskLevel,
    pub risk_reasons: Value,
    pub duplicate_candidates: Value,
}

impl fmt::Debug for CompletedTalentItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedTalentItem")
            .field("item_id", &self.item_id)
            .field("status", &self.status)
            .field("extraction", &self.extraction)
            .field("extraction_confidence", &self.extraction_confidence)
            .field("contact_candidates_ciphertext", &"[REDACTED]")
            .field("contact_hashes", &self.contact_hashes)
            .field("contact_masked", &self.contact_masked)
            .field("normalized_fingerprint", &self.normalized_fingerprint)
            .field("validation_errors", &self.validation_errors)
            .field("risk_score", &self.risk_score)
            .field("risk_level", &self.risk_level)
            .field("risk_reasons", &self.risk_reasons)
            .field("duplicate_candidates", &self.duplicate_candidates)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct FailedTalentItem {
    pub item_id: Uuid,
    pub validation_errors: Value,
}

#[derive(Serialize)]
struct NewTalentImportWire<'a> {
    imported_by: Option<Uuid>,
    raw_text_ciphertext: String,
    encryption_key_version: u32,
    model_provider: &'a Option<String>,
    model_name: &'a Option<String>,
    prompt_version: &'a Option<String>,
    raw_delete_after: Option<DateTime<Utc>>,
    items: Vec<NewTalentImportItemWire>,
}

impl<'a> From<&'a NewTalentImport> for NewTalentImportWire<'a> {
    fn from(import: &'a NewTalentImport) -> Self {
        Self {
            imported_by: import.imported_by,
            raw_text_ciphertext: hex_encode(&import.raw_text_ciphertext),
            encryption_key_version: import.encryption_key_version,
            model_provider: &import.model_provider,
            model_name: &import.model_name,
            prompt_version: &import.prompt_version,
            raw_delete_after: import.raw_delete_after,
            items: import.items.iter().map(NewTalentImportItemWire::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct NewTalentImportItemWire {
    position: u32,
    raw_fragment_ciphertext: String,
    encryption_key_version: u32,
    kind: TalentKind,
}

impl From<&NewTalentImportItem> for NewTalentImportItemWire {
    fn from(item: &NewTalentImportItem) -> Self {
        Self {
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
        })
    }
}

#[derive(Serialize)]
struct CompletedTalentItemWire<'a> {
    item_id: Uuid,
    status: ImportItemStatus,
    extraction: Value,
    extraction_confidence: f32,
    contact_candidates_ciphertext: Option<String>,
    contact_hashes: &'a [String],
    contact_masked: &'a Value,
    normalized_fingerprint: &'a Option<String>,
    validation_errors: &'a Value,
    risk_score: f32,
    risk_level: RiskLevel,
    risk_reasons: &'a Value,
    duplicate_candidates: &'a Value,
}

impl<'a> From<&'a CompletedTalentItem> for CompletedTalentItemWire<'a> {
    fn from(item: &'a CompletedTalentItem) -> Self {
        let extraction = match &item.extraction {
            Value::Object(fields) => {
                let mut fields = fields.clone();
                fields.remove("contacts");
                Value::Object(fields)
            }
            other => other.clone(),
        };
        Self {
            item_id: item.item_id,
            status: item.status,
            extraction,
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

#[async_trait]
pub(crate) trait DBTalent {
    async fn create_talent_import(&self, talent_import: NewTalentImport) -> Result<Uuid>;
    async fn list_talent_import_batches(&self, limit: u16) -> Result<Vec<TalentImportBatchSummary>>;
    async fn claim_talent_import_item(&self) -> Result<Option<ClaimedTalentItem>>;
    async fn complete_talent_import_item(&self, item: CompletedTalentItem) -> Result<()>;
    async fn fail_talent_import_item(&self, item: FailedTalentItem) -> Result<()>;
}

#[async_trait]
impl DBTalent for PgDB {
    #[instrument(skip(self, talent_import), err)]
    async fn create_talent_import(&self, talent_import: NewTalentImport) -> Result<Uuid> {
        trace!("db: create encrypted talent import");
        let wire = NewTalentImportWire::from(&talent_import);
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

fn hex_decode(encoded: &str) -> Result<Vec<u8>> {
    if encoded.len() % 2 != 0 {
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

    use crate::talent::{ImportItemStatus, RiskLevel, TalentKind};

    use super::{ClaimedTalentItemWire, NewTalentImport, NewTalentImportItem, NewTalentImportWire};

    #[test]
    fn import_wire_contains_only_hex_encoded_ciphertext() {
        let input = NewTalentImport {
            source_type: crate::talent::SourceType::Other,
            source_name: "Synthetic Source".to_owned(),
            imported_by: None,
            raw_text_ciphertext: vec![0, 1, 254, 255],
            encryption_key_version: 2,
            model_provider: Some("synthetic-provider".to_owned()),
            model_name: Some("synthetic-model".to_owned()),
            prompt_version: Some("talent-extract-v1".to_owned()),
            raw_delete_after: Some(Utc::now()),
            items: vec![NewTalentImportItem {
                position: 0,
                raw_fragment_ciphertext: vec![10, 11, 12],
                encryption_key_version: 2,
                kind: TalentKind::Hiring,
            }],
        };

        let wire = serde_json::to_value(NewTalentImportWire::from(&input)).unwrap();

        assert!(wire.get("source_id").is_none());
        assert_eq!(wire["raw_text_ciphertext"], "0001feff");
        assert_eq!(wire["items"][0]["raw_fragment_ciphertext"], "0a0b0c");
        assert!(wire.to_string().find("[0,1,254,255]").is_none());
    }

    #[test]
    fn claimed_item_wire_decodes_ciphertext_without_a_plaintext_field() {
        let item_id = Uuid::new_v4();
        let batch_id = Uuid::new_v4();
        let wire: ClaimedTalentItemWire = serde_json::from_value(json!({
            "item_id": item_id,
            "batch_id": batch_id,
            "position": 3,
            "raw_fragment_ciphertext": "0001feff",
            "encryption_key_version": 2,
            "kind": "hiring",
            "status": "extracting"
        }))
        .unwrap();

        let claimed = wire.try_into_claimed().unwrap();

        assert_eq!(claimed.item_id, item_id);
        assert_eq!(claimed.batch_id, batch_id);
        assert_eq!(claimed.raw_fragment_ciphertext, [0, 1, 254, 255]);
        assert_eq!(claimed.status, ImportItemStatus::Extracting);
    }

    #[test]
    fn completion_types_accept_safe_json_and_encrypted_contacts_only() {
        let completed = super::CompletedTalentItem {
            item_id: Uuid::new_v4(),
            status: ImportItemStatus::NeedsReview,
            extraction: json!({"kind": "hiring", "title": "Synthetic role"}),
            extraction_confidence: 0.75,
            contact_candidates_ciphertext: Some(vec![1, 2, 3]),
            contact_hashes: vec!["synthetic-hash".to_owned()],
            contact_masked: json!([{"contact_type": "wechat", "masked_value": "syn***001"}]),
            normalized_fingerprint: Some("synthetic-fingerprint".to_owned()),
            validation_errors: json!([]),
            risk_score: 10.0,
            risk_level: RiskLevel::Low,
            risk_reasons: json!([]),
            duplicate_candidates: json!([]),
        };

        let debug = format!("{completed:?}");
        assert!(!debug.contains("[1, 2, 3]"));
        assert!(debug.contains("[REDACTED]"));
    }
}
