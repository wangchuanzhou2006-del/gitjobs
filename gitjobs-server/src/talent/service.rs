use std::{collections::HashMap, fmt, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    config::TalentConfig,
    db::{
        DynDB,
        talent::{
            AuditReason, EncryptedTalentContact, ManualPolicyReason, MergeTalentImportItem, NewTalentImport,
            NewTalentImportItem, PublicTalentContact, PublishOutcome, PublishTalentItem, SaveReviewOutcome,
            SaveTalentReview, TalentDuplicateProbe, TalentRecordDraft, TalentReviewItem,
            TalentRuntimeSettings, TransitionTalentImportItem,
        },
    },
    talent::{
        AiError, AutoPublishInput, AutoPublishPolicy, ContactCandidate, DuplicateProbe, DuplicateService,
        EncryptedValue, ExtractionRequest, ImportItemStatus, OpenAiCompatibleProvider, RiskEngine, RiskInput,
        RuleEngine, SourceType, SplitError, SplitLimits, TalentAiProvider, TalentCrypto,
        TalentExtraction, TalentKind, ValidatedExtraction, ValidationIssue, contact_hash,
        duplicate_normalized_text, extraction_fingerprint, mask_contact, split_messages, validate_extraction,
    },
};

const RAW_RETENTION_DAYS: i64 = 90;
const DUPLICATE_LIMIT: u16 = 20;
const BATCH_AAD: &[u8] = b"talent-import:batch:v1";

pub(crate) struct TalentServices {
    pub import: Arc<ImportService>,
    pub review: Arc<ReviewService>,
    pub processor: Arc<crate::talent::TalentProcessor>,
}

impl TalentServices {
    pub(crate) fn from_config(
        db: DynDB,
        config: &TalentConfig,
        http_client: reqwest::Client,
    ) -> Result<Arc<Self>> {
        let (crypto, key_version) = crypto_from_config(config)?;
        let ai: Arc<dyn TalentAiProvider> = match config.ai_config() {
            Some(ai) => Arc::new(OpenAiCompatibleProvider::new(ai.clone(), http_client)),
            None => Arc::new(UnavailableTalentAiProvider),
        };
        let model_provider = config.ai_config().map(|_| "openai-compatible".to_owned());
        let model_name = config.ai_config().map(|ai| ai.model.clone());
        let prompt_version = config.ai_config().map(|_| "talent-extract-v1".to_owned());
        Ok(Arc::new(Self {
            import: Arc::new(ImportService::configured(
                db.clone(),
                crypto.clone(),
                key_version,
                config.enabled,
                model_provider,
                model_name,
                prompt_version,
            )),
            review: Arc::new(ReviewService::new(db.clone(), crypto.clone())),
            processor: Arc::new(crate::talent::TalentProcessor::new(db, crypto, ai)),
        }))
    }

    #[cfg(test)]
    pub(crate) fn for_tests(
        db: DynDB,
        crypto: Arc<TalentCrypto>,
        ai: Arc<dyn TalentAiProvider>,
    ) -> Arc<Self> {
        Arc::new(Self {
            import: Arc::new(ImportService::new(
                db.clone(),
                crypto.clone(),
                1,
                Some("mock".to_owned()),
                Some("mock-model".to_owned()),
                Some("test-v1".to_owned()),
            )),
            review: Arc::new(ReviewService::new(db.clone(), crypto.clone())),
            processor: Arc::new(crate::talent::TalentProcessor::new(db, crypto, ai)),
        })
    }
}

struct UnavailableTalentAiProvider;

#[async_trait]
impl TalentAiProvider for UnavailableTalentAiProvider {
    async fn extract(&self, _request: ExtractionRequest) -> Result<TalentExtraction, AiError> {
        Err(AiError::Transport)
    }
}

fn crypto_from_config(config: &TalentConfig) -> Result<(Arc<TalentCrypto>, u32)> {
    let configured_active_version = config
        .encryption
        .active_version
        .as_deref()
        .map(canonical_key_version)
        .transpose()
        .context("talent encryption active version must be a canonical positive integer")?;
    for version in config.encryption.keys.keys() {
        canonical_key_version(version)
            .context("talent encryption key version must be a canonical positive integer")?;
    }
    if !config.enabled {
        return Ok((
            Arc::new(
                TalentCrypto::new(
                    "1".to_owned(),
                    HashMap::from([(
                        "1".to_owned(),
                        "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
                    )]),
                    "disabled-talent-service".to_owned(),
                )
                .context("error creating disabled talent crypto")?,
            ),
            1,
        ));
    }
    let active = config
        .encryption
        .active_version
        .clone()
        .context("talent encryption active version is required")?;
    let key_version = configured_active_version.context("talent encryption active version is required")?;
    let keys = config
        .encryption
        .keys
        .iter()
        .map(|(version, secret)| (version.clone(), secret.expose_secret().to_owned()))
        .collect();
    let pepper = config
        .encryption
        .contact_pepper
        .as_ref()
        .context("talent contact pepper is required")?
        .expose_secret()
        .to_owned();
    let crypto = TalentCrypto::new(active, keys, pepper).context("error configuring talent crypto")?;
    Ok((Arc::new(crypto), key_version))
}

fn canonical_key_version(version: &str) -> Result<u32> {
    let parsed = version
        .parse::<u32>()
        .context("talent encryption key version must be an integer")?;
    if parsed == 0 || parsed.to_string() != version {
        anyhow::bail!("talent encryption key version must be canonical and positive");
    }
    Ok(parsed)
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct NewImport {
    pub source_type: SourceType,
    pub source_name: String,
    pub raw_text: String,
}

impl fmt::Debug for NewImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewImport")
            .field("source_type", &self.source_type)
            .field("source_name", &self.source_name)
            .field("raw_text", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImportReceipt {
    pub batch_id: Uuid,
    pub fragment_count: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ImportError {
    #[error("talent import is disabled")]
    Disabled,
    #[error("talent import is invalid")]
    InvalidInput,
    #[error("talent import encryption is unavailable")]
    Encryption,
    #[error("talent import could not be persisted")]
    Persistence,
}

pub(crate) struct ImportService {
    db: DynDB,
    crypto: Arc<TalentCrypto>,
    encryption_key_version: u32,
    enabled: bool,
    limits: SplitLimits,
    model_provider: Option<String>,
    model_name: Option<String>,
    prompt_version: Option<String>,
}

impl ImportService {
    pub(crate) fn new(
        db: DynDB,
        crypto: Arc<TalentCrypto>,
        encryption_key_version: u32,
        model_provider: Option<String>,
        model_name: Option<String>,
        prompt_version: Option<String>,
    ) -> Self {
        Self::configured(
            db,
            crypto,
            encryption_key_version,
            true,
            model_provider,
            model_name,
            prompt_version,
        )
    }

    fn configured(
        db: DynDB,
        crypto: Arc<TalentCrypto>,
        encryption_key_version: u32,
        enabled: bool,
        model_provider: Option<String>,
        model_name: Option<String>,
        prompt_version: Option<String>,
    ) -> Self {
        Self {
            db,
            crypto,
            encryption_key_version,
            enabled,
            limits: SplitLimits::default(),
            model_provider,
            model_name,
            prompt_version,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_limits(mut self, limits: SplitLimits) -> Self {
        self.limits = limits;
        self
    }

    pub(crate) async fn create_batch(
        &self,
        actor_id: Uuid,
        input: NewImport,
    ) -> Result<ImportReceipt, ImportError> {
        if !self.enabled {
            return Err(ImportError::Disabled);
        }
        let fragments = split_messages(&input.raw_text, self.limits).map_err(map_split_error)?;
        let batch = self
            .crypto
            .encrypt(input.raw_text.as_bytes(), batch_aad())
            .map_err(|_| ImportError::Encryption)?;
        let items = fragments
            .iter()
            .map(|fragment| {
                let kind = RuleEngine
                    .analyze(&fragment.text)
                    .kind_hint
                    .unwrap_or(TalentKind::Uncertain);
                let encrypted = self
                    .crypto
                    .encrypt(fragment.text.as_bytes(), &fragment_aad(fragment.position))
                    .map_err(|_| ImportError::Encryption)?;
                let encryption_key_version = self.checked_key_version(&encrypted)?;
                Ok(NewTalentImportItem {
                    position: u32::try_from(fragment.position).map_err(|_| ImportError::InvalidInput)?,
                    raw_fragment_ciphertext: encrypted.bytes,
                    encryption_key_version,
                    kind,
                })
            })
            .collect::<Result<Vec<_>, ImportError>>()?;
        let fragment_count = items.len();
        let encryption_key_version = self.checked_key_version(&batch)?;
        let talent_import = NewTalentImport {
            source_type: input.source_type,
            source_name: input.source_name,
            imported_by: Some(actor_id),
            raw_text_ciphertext: batch.bytes,
            encryption_key_version,
            model_provider: self.model_provider.clone(),
            model_name: self.model_name.clone(),
            prompt_version: self.prompt_version.clone(),
            raw_delete_after: Some(Utc::now() + TimeDelta::days(RAW_RETENTION_DAYS)),
            items,
        };
        let batch_id = self
            .db
            .create_talent_import(talent_import)
            .await
            .map_err(|_| ImportError::Persistence)?;

        Ok(ImportReceipt {
            batch_id,
            fragment_count,
        })
    }

    fn checked_key_version(&self, value: &EncryptedValue) -> Result<u32, ImportError> {
        let version = key_version(value)?;
        if version == self.encryption_key_version {
            Ok(version)
        } else {
            Err(ImportError::Encryption)
        }
    }
}

fn map_split_error(_: SplitError) -> ImportError {
    ImportError::InvalidInput
}

fn key_version(value: &EncryptedValue) -> Result<u32, ImportError> {
    canonical_key_version(&value.key_version).map_err(|_| ImportError::Encryption)
}

pub(crate) const fn batch_aad() -> &'static [u8] {
    BATCH_AAD
}

pub(crate) fn fragment_aad(position: usize) -> Vec<u8> {
    format!("talent-import:fragment:v1:{position}").into_bytes()
}

pub(crate) fn contacts_aad(item_id: Uuid) -> Vec<u8> {
    format!("talent-import:contacts:v1:{item_id}").into_bytes()
}

pub(crate) fn contact_aad(item_id: Uuid, index: usize) -> Vec<u8> {
    format!("talent-contact:v1:{item_id}:{index}").into_bytes()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ReviewError {
    #[error("talent review item was not found")]
    NotFound,
    #[error("talent review item is incomplete")]
    InvalidState,
    #[error("talent review item contains invalid structured data")]
    InvalidExtraction,
    #[error("talent review protected data is unavailable")]
    ProtectedData,
    #[error("talent review operation could not be persisted")]
    Persistence,
}

pub(crate) struct ReviewService {
    db: DynDB,
    crypto: Arc<TalentCrypto>,
    duplicate: DuplicateService,
    risk: RiskEngine,
    policy: AutoPublishPolicy,
}

impl ReviewService {
    pub(crate) fn new(db: DynDB, crypto: Arc<TalentCrypto>) -> Self {
        Self {
            db,
            crypto,
            duplicate: DuplicateService,
            risk: RiskEngine,
            policy: AutoPublishPolicy,
        }
    }

    pub(crate) async fn save(&self, review: SaveTalentReview) -> Result<SaveReviewOutcome, ReviewError> {
        self.db
            .save_talent_review(review)
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    pub(crate) async fn publish(
        &self,
        item_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<PublishOutcome, ReviewError> {
        let item = self.review_item(item_id).await?;
        let (validated, hints) = self.revalidate(&item)?;
        let normalized_fingerprint = extraction_fingerprint(&validated.safe);
        let candidates = self
            .db
            .find_talent_duplicate_candidates(
                duplicate_repository_probe(
                    item.item_id,
                    item.source_id,
                    item.contact_hashes.clone(),
                    normalized_fingerprint.clone(),
                    &validated,
                ),
                DUPLICATE_LIMIT,
            )
            .await
            .map_err(|_| ReviewError::Persistence)?;
        let duplicates = self
            .duplicate
            .evaluate(DuplicateProbe::new(validated.clone(), item.source_id), candidates);
        let model_rule_conflict = hints.conflicts_with(validated.safe.kind);
        let assessment = self.risk.evaluate(RiskInput {
            extraction: validated.clone(),
            source_trust: item.source_trust_status,
            rule_hints: hints,
            duplicates: duplicates.clone(),
            identity_anomaly: false,
            model_rule_conflict,
        });
        let settings = self
            .db
            .get_talent_runtime_settings()
            .await
            .map_err(|_| ReviewError::Persistence)?;
        let server_decision = self.policy.decide(AutoPublishInput {
            global_enabled: settings.auto_publish_enabled,
            source_trust: item.source_trust_status,
            source_enabled: item.source_auto_publish_enabled,
            source_confirmed: item.source_confirmed_at.is_some(),
            assessment: assessment.clone(),
            extraction: validated.clone(),
            duplicates: duplicates.clone(),
        });
        let masked = masked_contacts(&validated.contacts)?;
        let save = SaveTalentReview {
            item_id,
            lock_version: item.lock_version,
            actor_user_id,
            kind: Some(validated.safe.kind),
            extraction: Some(
                crate::db::talent::SafeTalentExtraction::try_from(validated.safe.clone())
                    .map_err(|_| ReviewError::InvalidExtraction)?,
            ),
            extraction_confidence: Some(validated.safe.confidence),
            contact_masked: Some(masked),
            validation_errors: Some(json!([])),
            risk_score: Some(f32::from(assessment.score)),
            risk_level: Some(assessment.level),
            risk_reasons: Some(
                serde_json::to_value(&assessment.reasons).map_err(|_| ReviewError::InvalidExtraction)?,
            ),
            duplicate_candidates: Some(duplicates),
            normalized_fingerprint,
            manual_policy_reasons: Some(
                server_decision
                    .reasons
                    .iter()
                    .filter_map(|reason| ManualPolicyReason::from_code(&reason.code))
                    .collect(),
            ),
        };
        let lock_version = match self.save(save).await? {
            SaveReviewOutcome::Saved { lock_version } => lock_version,
            SaveReviewOutcome::Conflict => return Err(ReviewError::InvalidState),
        };
        let contacts = encrypt_publication_contacts(&self.crypto, item_id, &validated.contacts)?;
        let record = record_draft(&item, &validated.safe)?;

        self.db
            .publish_talent_item(PublishTalentItem {
                item_id,
                lock_version,
                actor_user_id: Some(actor_user_id),
                automatic: false,
                record,
                contacts,
            })
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    pub(crate) async fn reject(
        &self,
        item_id: Uuid,
        lock_version: u32,
        actor_user_id: Uuid,
        reason: Option<AuditReason>,
    ) -> Result<SaveReviewOutcome, ReviewError> {
        self.transition(
            item_id,
            lock_version,
            actor_user_id,
            ImportItemStatus::Rejected,
            reason.or(Some(AuditReason::Rejected)),
        )
        .await
    }

    pub(crate) async fn merge(&self, merge: MergeTalentImportItem) -> Result<SaveReviewOutcome, ReviewError> {
        self.db
            .merge_talent_import_item(merge)
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    pub(crate) async fn reprocess(
        &self,
        item_id: Uuid,
        lock_version: u32,
        actor_user_id: Uuid,
    ) -> Result<SaveReviewOutcome, ReviewError> {
        self.transition(
            item_id,
            lock_version,
            actor_user_id,
            ImportItemStatus::Queued,
            Some(AuditReason::Reprocess),
        )
        .await
    }

    pub(crate) async fn withdraw(
        &self,
        record_id: Uuid,
        actor_user_id: Uuid,
        reason: Option<AuditReason>,
    ) -> Result<bool, ReviewError> {
        self.db
            .withdraw_talent_record(record_id, actor_user_id, reason.or(Some(AuditReason::Withdrawn)))
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    pub(crate) async fn pause_auto_publish(
        &self,
        actor_user_id: Uuid,
    ) -> Result<TalentRuntimeSettings, ReviewError> {
        self.db
            .set_talent_auto_publish(false, actor_user_id)
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    async fn transition(
        &self,
        item_id: Uuid,
        lock_version: u32,
        actor_user_id: Uuid,
        next_status: ImportItemStatus,
        reason: Option<AuditReason>,
    ) -> Result<SaveReviewOutcome, ReviewError> {
        self.db
            .transition_talent_import_item(TransitionTalentImportItem {
                item_id,
                lock_version,
                next_status,
                actor_user_id,
                reason,
            })
            .await
            .map_err(|_| ReviewError::Persistence)
    }

    async fn review_item(&self, item_id: Uuid) -> Result<TalentReviewItem, ReviewError> {
        self.db
            .get_talent_review_item(item_id)
            .await
            .map_err(|_| ReviewError::Persistence)?
            .ok_or(ReviewError::NotFound)
    }

    fn revalidate(
        &self,
        item: &TalentReviewItem,
    ) -> Result<(ValidatedExtraction, crate::talent::RuleHints), ReviewError> {
        let contacts = decrypt_review_contacts(&self.crypto, item)?;
        let mut value = item.extraction.clone().ok_or(ReviewError::InvalidState)?;
        value.as_object_mut().ok_or(ReviewError::InvalidExtraction)?.insert(
            "contacts".to_owned(),
            serde_json::to_value(contacts).map_err(|_| ReviewError::InvalidExtraction)?,
        );
        let extraction: TalentExtraction =
            serde_json::from_value(value).map_err(|_| ReviewError::InvalidExtraction)?;
        let validated = validate_extraction(extraction).map_err(|_| ReviewError::InvalidExtraction)?;
        let hints = match (item.raw_fragment_ciphertext.as_ref(), item.encryption_key_version) {
            (Some(bytes), Some(version)) => {
                let position = usize::try_from(item.position).map_err(|_| ReviewError::InvalidState)?;
                let plaintext = self
                    .crypto
                    .decrypt(
                        &EncryptedValue {
                            key_version: version.to_string(),
                            bytes: bytes.clone(),
                        },
                        &fragment_aad(position),
                    )
                    .map_err(|_| ReviewError::ProtectedData)?;
                let fragment = String::from_utf8(plaintext).map_err(|_| ReviewError::ProtectedData)?;
                RuleEngine.analyze(&fragment)
            }
            _ => crate::talent::RuleHints::default(),
        };
        Ok((validated, hints))
    }
}

pub(crate) fn safe_snapshot(extraction: &TalentExtraction) -> Value {
    let mut snapshot = serde_json::to_value(extraction).unwrap_or_else(|_| json!({}));
    let masked = extraction
        .contacts
        .iter()
        .map(|contact| mask_contact(contact.contact_type, &contact.value))
        .collect::<Vec<_>>();
    if let Some(object) = snapshot.as_object_mut() {
        object.insert(
            "contacts".to_owned(),
            json!({
                "present": !extraction.contacts.is_empty(),
                "masked": masked,
            }),
        );
    }
    snapshot
}

pub(crate) fn masked_contacts(
    contacts: &[ContactCandidate],
) -> Result<Vec<PublicTalentContact>, ReviewError> {
    contacts
        .iter()
        .map(|contact| {
            Ok(PublicTalentContact {
                contact_type: contact.contact_type,
                masked_value: mask_contact(contact.contact_type, &contact.value)
                    .try_into()
                    .map_err(|_| ReviewError::InvalidExtraction)?,
            })
        })
        .collect()
}

pub(crate) fn encrypt_publication_contacts(
    crypto: &TalentCrypto,
    item_id: Uuid,
    contacts: &[ContactCandidate],
) -> Result<Vec<EncryptedTalentContact>, ReviewError> {
    contacts
        .iter()
        .enumerate()
        .map(|(index, contact)| {
            let encrypted = crypto
                .encrypt(contact.value.as_bytes(), &contact_aad(item_id, index))
                .map_err(|_| ReviewError::ProtectedData)?;
            Ok(EncryptedTalentContact {
                contact_type: contact.contact_type,
                ciphertext: encrypted.bytes,
                encryption_key_version: canonical_key_version(&encrypted.key_version)
                    .map_err(|_| ReviewError::ProtectedData)?,
                normalized_hash: Some(contact_hash(crypto, contact).map_err(|_| ReviewError::ProtectedData)?),
                masked_value: mask_contact(contact.contact_type, &contact.value)
                    .try_into()
                    .map_err(|_| ReviewError::InvalidExtraction)?,
            })
        })
        .collect()
}

pub(crate) fn decrypt_review_contacts(
    crypto: &TalentCrypto,
    item: &TalentReviewItem,
) -> Result<Vec<ContactCandidate>, ReviewError> {
    let Some(bytes) = &item.contact_candidates_ciphertext else {
        return Ok(Vec::new());
    };
    let version = item.encryption_key_version.ok_or(ReviewError::ProtectedData)?;
    let plaintext = crypto
        .decrypt(
            &EncryptedValue {
                key_version: version.to_string(),
                bytes: bytes.clone(),
            },
            &contacts_aad(item.item_id),
        )
        .map_err(|_| ReviewError::ProtectedData)?;
    serde_json::from_slice(&plaintext).map_err(|_| ReviewError::ProtectedData)
}

pub(crate) fn record_draft(
    item: &TalentReviewItem,
    extraction: &TalentExtraction,
) -> Result<TalentRecordDraft, ReviewError> {
    Ok(TalentRecordDraft {
        kind: extraction.kind,
        title: extraction.title.clone().ok_or(ReviewError::InvalidState)?,
        scene: extraction.scene.clone(),
        region: extraction.region.clone(),
        city: extraction.city.clone(),
        district: extraction.district.clone(),
        salary_min: extraction.salary_min,
        salary_max: extraction.salary_max,
        roles: extraction.roles.clone(),
        skills: extraction.skills.clone(),
        experience_years: extraction.experience_years,
        hours_per_day: extraction.hours_per_day,
        rest_days_per_month: extraction.rest_days_per_month,
        lodging: extraction.lodging.as_deref().and_then(parse_bool),
        housing_allowance: extraction.housing_allowance.map(|value| value > 0),
        availability: extraction.availability.clone(),
        urgency: extraction.urgency.clone(),
        source_label: Some(item.source_name.clone()),
        published_at: extraction.published_at,
        expires_at: None,
    })
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "provided" | "1" => Some(true),
        "false" | "no" | "none" | "0" => Some(false),
        _ => None,
    }
}

pub(crate) fn validation_issues(issues: &[ValidationIssue]) -> Value {
    Value::Array(
        issues
            .iter()
            .map(|issue| json!({"code": issue.code, "field": issue.field}))
            .collect(),
    )
}

pub(crate) fn duplicate_repository_probe(
    item_id: Uuid,
    source_id: Uuid,
    contact_hashes: Vec<String>,
    normalized_fingerprint: Option<String>,
    extraction: &ValidatedExtraction,
) -> TalentDuplicateProbe {
    TalentDuplicateProbe {
        item_id,
        kind: extraction.safe.kind,
        title: extraction.safe.title.clone(),
        roles: extraction.safe.roles.clone(),
        city: extraction.safe.city.clone(),
        district: extraction.safe.district.clone(),
        salary_min: extraction.safe.salary_min,
        salary_max: extraction.safe.salary_max,
        source_id,
        contact_hashes,
        normalized_fingerprint,
        normalized_text: duplicate_normalized_text(&extraction.safe),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use mockall::Sequence;
    use uuid::Uuid;

    use crate::{
        config::{SecretValue, TalentConfig, TalentEncryptionConfig},
        db::{
            mock::MockDB,
            talent::{
                ManualPolicyReason, PublishOutcome, SaveReviewOutcome, TalentReviewItem,
                TalentRuntimeSettings,
            },
        },
        talent::{
            AiError, ContactCandidate, ContactType, ExtractionRequest, ImportItemStatus, SourceTrust,
            SourceType, SplitLimits, TalentAiProvider, TalentCrypto, TalentExtraction, TalentKind,
            TalentServices,
        },
    };

    use super::{
        ImportError, ImportService, NewImport, ReviewService, batch_aad, canonical_key_version, contacts_aad,
        crypto_from_config, fragment_aad, safe_snapshot,
    };

    fn crypto() -> Arc<TalentCrypto> {
        Arc::new(
            TalentCrypto::new(
                "1".to_owned(),
                HashMap::from([(
                    "1".to_owned(),
                    "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_owned(),
                )]),
                "test-contact-pepper".to_owned(),
            )
            .unwrap(),
        )
    }

    fn new_import(raw_text: &str) -> NewImport {
        NewImport {
            source_type: SourceType::WechatGroup,
            source_name: "synthetic source".to_owned(),
            raw_text: raw_text.to_owned(),
        }
    }

    struct CountingAi(AtomicUsize);

    #[async_trait]
    impl TalentAiProvider for CountingAi {
        async fn extract(&self, _request: ExtractionRequest) -> Result<TalentExtraction, AiError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(TalentExtraction::default())
        }
    }

    #[tokio::test]
    async fn validates_split_limits_before_any_database_write() {
        let mut db = MockDB::new();
        db.expect_create_talent_import().never();
        let service =
            ImportService::new(Arc::new(db), crypto(), 1, None, None, None).with_limits(SplitLimits {
                max_raw_bytes: 3,
                ..SplitLimits::default()
            });

        let error = service
            .create_batch(Uuid::new_v4(), new_import("four"))
            .await
            .unwrap_err();

        assert!(matches!(error, ImportError::InvalidInput));
    }

    #[tokio::test]
    async fn encrypts_batch_and_fragments_with_distinct_aad() {
        let crypto = crypto();
        let decryptor = crypto.clone();
        let mut db = MockDB::new();
        db.expect_create_talent_import()
            .times(1)
            .withf(move |import| {
                let batch = crate::talent::EncryptedValue {
                    key_version: import.encryption_key_version.to_string(),
                    bytes: import.raw_text_ciphertext.clone(),
                };
                let first = crate::talent::EncryptedValue {
                    key_version: import.items[0].encryption_key_version.to_string(),
                    bytes: import.items[0].raw_fragment_ciphertext.clone(),
                };
                let second = crate::talent::EncryptedValue {
                    key_version: import.items[1].encryption_key_version.to_string(),
                    bytes: import.items[1].raw_fragment_ciphertext.clone(),
                };
                decryptor
                    .decrypt(&batch, batch_aad())
                    .is_ok_and(|plaintext| plaintext == b"first\n\n\nsecond")
                    && decryptor.decrypt(&batch, &fragment_aad(0)).is_err()
                    && decryptor
                        .decrypt(&first, &fragment_aad(0))
                        .is_ok_and(|plaintext| plaintext == b"first")
                    && decryptor.decrypt(&first, batch_aad()).is_err()
                    && decryptor
                        .decrypt(&second, &fragment_aad(1))
                        .is_ok_and(|plaintext| plaintext == b"second")
                    && decryptor.decrypt(&second, &fragment_aad(0)).is_err()
            })
            .returning(|_| Ok(Uuid::nil()));
        let service = ImportService::new(Arc::new(db), crypto, 1, None, None, None);

        let receipt = service
            .create_batch(Uuid::new_v4(), new_import("first\n\n\nsecond"))
            .await
            .unwrap();

        assert_eq!(receipt.batch_id, Uuid::nil());
        assert_eq!(receipt.fragment_count, 2);
    }

    #[tokio::test]
    async fn http_facing_import_never_calls_the_ai_provider() {
        let mut db = MockDB::new();
        db.expect_create_talent_import()
            .times(1)
            .returning(|_| Ok(Uuid::new_v4()));
        let ai = Arc::new(CountingAi(AtomicUsize::new(0)));
        let services = TalentServices::for_tests(Arc::new(db), crypto(), ai.clone());

        services
            .import
            .create_batch(Uuid::new_v4(), new_import("synthetic import"))
            .await
            .unwrap();

        assert_eq!(ai.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn safe_snapshot_masks_and_replaces_complete_contacts() {
        let complete = "13800138000";
        let snapshot = safe_snapshot(&TalentExtraction {
            kind: TalentKind::Seeking,
            contacts: vec![ContactCandidate {
                contact_type: ContactType::Phone,
                value: complete.to_owned(),
            }],
            ..TalentExtraction::default()
        });

        assert_eq!(snapshot["contacts"]["present"], true);
        assert_eq!(snapshot["contacts"]["masked"][0], "138****8000");
        assert!(!snapshot.to_string().contains(complete));
    }

    #[test]
    fn key_versions_are_canonical_positive_u32_values() {
        assert_eq!(canonical_key_version("1").unwrap(), 1);
        assert_eq!(
            canonical_key_version(u32::MAX.to_string().as_str()).unwrap(),
            u32::MAX
        );
        for invalid in ["", "0", "01", "+1", "-1", " 1", "4294967296", "version-1"] {
            assert!(canonical_key_version(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn configured_historic_key_versions_must_also_be_canonical() {
        let config = TalentConfig {
            enabled: true,
            encryption: TalentEncryptionConfig {
                active_version: Some("2".to_owned()),
                keys: [
                    ("2".to_owned(), SecretValue::new("22".repeat(32))),
                    ("01".to_owned(), SecretValue::new("11".repeat(32))),
                ]
                .into_iter()
                .collect(),
                contact_pepper: Some(SecretValue::new("synthetic-test-pepper")),
            },
            ..TalentConfig::default()
        };

        assert!(crypto_from_config(&config).is_err());
    }

    #[tokio::test]
    async fn reprocess_queues_the_item_for_the_normal_claim_path() {
        let item_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut db = MockDB::new();
        db.expect_transition_talent_import_item()
            .times(1)
            .withf(move |transition| {
                transition.item_id == item_id
                    && transition.lock_version == 7
                    && transition.actor_user_id == actor_id
                    && transition.next_status == crate::talent::ImportItemStatus::Queued
            })
            .returning(|_| Ok(crate::db::talent::SaveReviewOutcome::Saved { lock_version: 8 }));
        let review = ReviewService::new(Arc::new(db), crypto());

        assert_eq!(
            review.reprocess(item_id, 7, actor_id).await.unwrap(),
            crate::db::talent::SaveReviewOutcome::Saved { lock_version: 8 }
        );
    }

    #[tokio::test]
    async fn manual_publish_rechecks_policy_and_audits_fixed_override_reasons() {
        let item_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let contact = ContactCandidate {
            contact_type: ContactType::Wechat,
            value: "candidate_2026".to_owned(),
        };
        let extraction = TalentExtraction {
            kind: TalentKind::Hiring,
            confidence: 0.95,
            title: Some("Cook".to_owned()),
            city: Some("Test City".to_owned()),
            salary_min: Some(6_000),
            salary_max: Some(8_000),
            roles: vec!["Cook".to_owned()],
            published_at: Some(Utc::now()),
            contacts: vec![contact.clone()],
            ..TalentExtraction::default()
        };
        let expected_fingerprint =
            crate::talent::extraction_fingerprint(&extraction).expect("complete extraction fingerprint");
        assert_ne!(expected_fingerprint, "b".repeat(64));
        let crypto = crypto();
        let protected_contacts = crypto
            .encrypt_with_version(
                &serde_json::to_vec(&[contact]).unwrap(),
                &contacts_aad(item_id),
                "1",
            )
            .unwrap();
        let mut safe_extraction = serde_json::to_value(&extraction).unwrap();
        safe_extraction.as_object_mut().unwrap().remove("contacts");
        let review_item = TalentReviewItem {
            item_id,
            batch_id: Uuid::new_v4(),
            position: 0,
            raw_fragment_ciphertext: None,
            encryption_key_version: Some(1),
            contact_candidates_ciphertext: Some(protected_contacts.bytes),
            contact_hashes: vec!["a".repeat(64)],
            contact_masked: None,
            normalized_fingerprint: Some("b".repeat(64)),
            kind: TalentKind::Hiring,
            status: ImportItemStatus::NeedsReview,
            extraction: Some(safe_extraction),
            extraction_confidence: Some(0.95),
            validation_errors: Some(serde_json::json!([])),
            risk_score: Some(20.0),
            risk_level: Some(crate::talent::RiskLevel::Low),
            risk_reasons: Some(serde_json::json!([])),
            duplicate_candidates: Some(serde_json::json!([])),
            published_record_id: None,
            lock_version: 4,
            processed_at: Some(Utc::now()),
            reviewed_at: None,
            updated_at: Utc::now(),
            model_provider: Some("mock".to_owned()),
            model_name: Some("mock-model".to_owned()),
            prompt_version: Some("v1".to_owned()),
            source_id,
            source_type: SourceType::WechatGroup,
            source_name: "synthetic source".to_owned(),
            source_trust_status: SourceTrust::New,
            source_auto_publish_enabled: true,
            source_confirmed_at: Some(Utc::now()),
        };
        let mut db = MockDB::new();
        let mut sequence = Sequence::new();
        db.expect_get_talent_review_item()
            .times(1)
            .in_sequence(&mut sequence)
            .return_once(move |_| Ok(Some(review_item)));
        db.expect_find_talent_duplicate_candidates()
            .times(1)
            .in_sequence(&mut sequence)
            .withf({
                let expected_fingerprint = expected_fingerprint.clone();
                move |probe, limit| {
                    probe.item_id == item_id
                        && probe.source_id == source_id
                        && probe.contact_hashes == ["a".repeat(64)]
                        && probe.normalized_fingerprint.as_deref() == Some(expected_fingerprint.as_str())
                        && *limit == 20
                }
            })
            .returning(|_, _| Ok(vec![]));
        db.expect_get_talent_runtime_settings()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|| {
                Ok(TalentRuntimeSettings {
                    auto_publish_enabled: true,
                    updated_by: None,
                    updated_at: Utc::now(),
                })
            });
        db.expect_save_talent_review()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |review| {
                review.item_id == item_id
                    && review.actor_user_id == actor_id
                    && review.normalized_fingerprint.as_deref() == Some(expected_fingerprint.as_str())
                    && review.manual_policy_reasons == Some(vec![ManualPolicyReason::SourceNotTrusted])
            })
            .returning(|_| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        db.expect_publish_talent_item()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(move |publication| {
                publication.item_id == item_id
                    && publication.lock_version == 5
                    && publication.actor_user_id == Some(actor_id)
                    && !publication.automatic
            })
            .returning(|_| {
                Ok(PublishOutcome {
                    record_id: Uuid::new_v4(),
                    created: true,
                })
            });
        let review = ReviewService::new(Arc::new(db), crypto);

        assert!(review.publish(item_id, actor_id).await.unwrap().created);
    }
}
