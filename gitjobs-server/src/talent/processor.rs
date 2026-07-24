use std::sync::Arc;

use serde_json::json;
use thiserror::Error;

use crate::{
    db::{
        DynDB,
        talent::{
            ClaimedTalentItem, CompletedTalentItem, FailedTalentItem, PublishTalentItem,
            SafeTalentExtraction, TalentReviewItem,
        },
    },
    talent::{
        AiError, AutoPublishInput, AutoPublishPolicy, ContactCandidate, DuplicateProbe, DuplicateService,
        EncryptedValue, ExtractionRequest, RiskEngine, RiskInput, RuleEngine, TalentAiProvider, TalentCrypto,
        TalentExtraction, ValidatedExtraction, ValidationIssue, contact_hash, validate_extraction,
    },
};

use super::service::{
    ReviewError, contacts_aad, encrypt_publication_contacts, fragment_aad, masked_contacts, record_draft,
    validation_issues,
};

const DUPLICATE_LIMIT: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessOutcome {
    NoWork,
    AutoPublished,
    NeedsReview,
    Failed,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ProcessError {
    #[error("talent processing persistence failed")]
    Persistence,
    #[error("talent processing protected data failed")]
    ProtectedData,
    #[error("talent processing structured data failed")]
    StructuredData,
}

pub(crate) struct TalentProcessor {
    db: DynDB,
    crypto: Arc<TalentCrypto>,
    ai: Arc<dyn TalentAiProvider>,
    rules: RuleEngine,
    duplicate: DuplicateService,
    risk: RiskEngine,
    policy: AutoPublishPolicy,
}

impl TalentProcessor {
    pub(crate) fn new(db: DynDB, crypto: Arc<TalentCrypto>, ai: Arc<dyn TalentAiProvider>) -> Self {
        Self {
            db,
            crypto,
            ai,
            rules: RuleEngine,
            duplicate: DuplicateService,
            risk: RiskEngine,
            policy: AutoPublishPolicy,
        }
    }

    pub(crate) async fn process_one(&self) -> Result<ProcessOutcome, ProcessError> {
        let Some(claimed) = self
            .db
            .claim_talent_import_item()
            .await
            .map_err(|_| ProcessError::Persistence)?
        else {
            return Ok(ProcessOutcome::NoWork);
        };
        let fragment = match self.decrypt_fragment(&claimed) {
            Some(fragment) => fragment,
            None => {
                self.fail_safely(claimed.item_id, "decrypt_failed").await?;
                return Ok(ProcessOutcome::Failed);
            }
        };
        let metadata = match self
            .db
            .get_talent_review_item(claimed.item_id)
            .await
            .map_err(|_| ProcessError::Persistence)?
        {
            Some(item) => item,
            None => {
                self.fail_safely(claimed.item_id, "item_metadata_missing").await?;
                return Ok(ProcessOutcome::Failed);
            }
        };
        let hints = self.rules.analyze(&fragment);
        let extraction = match self.extract_with_one_timeout_retry(fragment).await {
            Ok(extraction) => extraction,
            Err(code) => {
                let validated = unavailable_extraction(code);
                self.persist_review(&claimed, &metadata, validated, hints, false)
                    .await?;
                return Ok(ProcessOutcome::NeedsReview);
            }
        };
        let validated = match validate_extraction(extraction.clone()) {
            Ok(validated) => validated,
            Err(issues) => invalid_extraction(extraction, issues),
        };
        let strictly_valid = validated.issues.is_empty();
        let eligible = match self
            .persist_review(&claimed, &metadata, validated.clone(), hints, strictly_valid)
            .await
        {
            Ok(eligible) => eligible,
            Err(ProcessError::ProtectedData | ProcessError::StructuredData) => {
                self.fail_safely(claimed.item_id, "processing_failed").await?;
                return Ok(ProcessOutcome::Failed);
            }
            Err(error) => return Err(error),
        };

        if eligible {
            let contacts = encrypt_publication_contacts(&self.crypto, claimed.item_id, &validated.contacts)
                .map_err(map_review_error)?;
            let record = record_draft(&metadata, &validated.safe).map_err(map_review_error)?;
            self.db
                .publish_talent_item(PublishTalentItem {
                    item_id: claimed.item_id,
                    lock_version: metadata.lock_version,
                    actor_user_id: None,
                    automatic: true,
                    record,
                    contacts,
                })
                .await
                .map_err(|_| ProcessError::Persistence)?;
            Ok(ProcessOutcome::AutoPublished)
        } else {
            Ok(ProcessOutcome::NeedsReview)
        }
    }

    fn decrypt_fragment(&self, claimed: &ClaimedTalentItem) -> Option<String> {
        let position = usize::try_from(claimed.position).ok()?;
        let plaintext = self
            .crypto
            .decrypt(
                &EncryptedValue {
                    key_version: claimed.encryption_key_version.to_string(),
                    bytes: claimed.raw_fragment_ciphertext.clone(),
                },
                &fragment_aad(position),
            )
            .ok()?;
        String::from_utf8(plaintext).ok()
    }

    async fn extract_with_one_timeout_retry(
        &self,
        fragment: String,
    ) -> Result<TalentExtraction, &'static str> {
        match self.ai.extract(ExtractionRequest::new(fragment.clone())).await {
            Ok(extraction) => Ok(extraction),
            Err(AiError::Timeout) => self
                .ai
                .extract(ExtractionRequest::new(fragment))
                .await
                .map_err(ai_error_code),
            Err(error) => Err(ai_error_code(error)),
        }
    }

    async fn persist_review(
        &self,
        claimed: &ClaimedTalentItem,
        metadata: &TalentReviewItem,
        validated: ValidatedExtraction,
        hints: crate::talent::RuleHints,
        strictly_valid: bool,
    ) -> Result<bool, ProcessError> {
        let protected = protect_contacts(
            &self.crypto,
            claimed.item_id,
            claimed.encryption_key_version,
            &validated.contacts,
        )?;
        let candidates = self
            .db
            .find_talent_duplicate_candidates(claimed.item_id, DUPLICATE_LIMIT)
            .await
            .map_err(|_| ProcessError::Persistence)?;
        let duplicates = self.duplicate.evaluate(
            DuplicateProbe::new(validated.clone(), metadata.source_id),
            candidates,
        );
        let model_rule_conflict = hints.conflicts_with(validated.safe.kind);
        let assessment = self.risk.evaluate(RiskInput {
            extraction: validated.clone(),
            source_trust: metadata.source_trust_status,
            rule_hints: hints,
            duplicates: duplicates.clone(),
            identity_anomaly: false,
            model_rule_conflict,
        });
        let settings = self
            .db
            .get_talent_runtime_settings()
            .await
            .map_err(|_| ProcessError::Persistence)?;
        let decision = self.policy.decide(AutoPublishInput {
            global_enabled: settings.auto_publish_enabled,
            source_trust: metadata.source_trust_status,
            source_enabled: metadata.source_auto_publish_enabled,
            source_confirmed: metadata.source_confirmed_at.is_some(),
            assessment: assessment.clone(),
            extraction: validated.clone(),
            duplicates: duplicates.clone(),
        });
        let extraction = SafeTalentExtraction::try_from(validated.safe.clone())
            .map_err(|_| ProcessError::StructuredData)?;
        let masked = masked_contacts(&validated.contacts).map_err(map_review_error)?;
        self.db
            .complete_talent_import_item(CompletedTalentItem {
                item_id: claimed.item_id,
                extraction,
                extraction_confidence: validated.safe.confidence,
                contact_candidates_ciphertext: protected.ciphertext,
                contact_hashes: protected.hashes,
                contact_masked: masked,
                normalized_fingerprint: extraction_fingerprint(&validated.safe),
                validation_errors: validation_issues(&validated.issues),
                risk_score: f32::from(assessment.score),
                risk_level: assessment.level,
                risk_reasons: serde_json::to_value(&assessment.reasons)
                    .map_err(|_| ProcessError::StructuredData)?,
                duplicate_candidates: duplicates,
            })
            .await
            .map_err(|_| ProcessError::Persistence)?;
        Ok(strictly_valid && decision.eligible)
    }

    async fn fail_safely(&self, item_id: uuid::Uuid, code: &str) -> Result<(), ProcessError> {
        self.db
            .fail_talent_import_item(FailedTalentItem {
                item_id,
                validation_errors: json!([{"code": code}]),
            })
            .await
            .map_err(|_| ProcessError::Persistence)
    }
}

struct ProtectedContacts {
    ciphertext: Option<Vec<u8>>,
    hashes: Vec<String>,
}

fn protect_contacts(
    crypto: &TalentCrypto,
    item_id: uuid::Uuid,
    encryption_key_version: u32,
    contacts: &[ContactCandidate],
) -> Result<ProtectedContacts, ProcessError> {
    let ciphertext = if contacts.is_empty() {
        None
    } else {
        let plaintext = serde_json::to_vec(contacts).map_err(|_| ProcessError::StructuredData)?;
        Some(
            crypto
                .encrypt_with_version(
                    &plaintext,
                    &contacts_aad(item_id),
                    &encryption_key_version.to_string(),
                )
                .map_err(|_| ProcessError::ProtectedData)?
                .bytes,
        )
    };
    let hashes = contacts
        .iter()
        .map(|contact| contact_hash(crypto, contact).map_err(|_| ProcessError::ProtectedData))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProtectedContacts { ciphertext, hashes })
}

fn unavailable_extraction(code: &'static str) -> ValidatedExtraction {
    ValidatedExtraction {
        safe: TalentExtraction::default(),
        contacts: Vec::new(),
        issues: vec![ValidationIssue {
            code: code.to_owned(),
            field: "extraction".to_owned(),
        }],
    }
}

fn invalid_extraction(_extraction: TalentExtraction, issues: Vec<ValidationIssue>) -> ValidatedExtraction {
    ValidatedExtraction {
        safe: TalentExtraction::default(),
        contacts: Vec::new(),
        issues,
    }
}

fn ai_error_code(error: AiError) -> &'static str {
    match error {
        AiError::Timeout => "ai_timeout",
        AiError::Transport | AiError::HttpStatus(_) => "ai_unavailable",
        AiError::InvalidResponse | AiError::MissingChoice | AiError::InvalidJson | AiError::InvalidSchema => {
            "ai_invalid_response"
        }
    }
}

fn extraction_fingerprint(extraction: &TalentExtraction) -> Option<String> {
    let normalized = extraction
        .title
        .iter()
        .chain(extraction.roles.iter())
        .chain(extraction.city.iter())
        .chain(extraction.district.iter())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\u{0}");
    if normalized.is_empty() {
        return None;
    }
    Some(
        openssl::sha::sha256(normalized.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn map_review_error(error: ReviewError) -> ProcessError {
    match error {
        ReviewError::ProtectedData => ProcessError::ProtectedData,
        ReviewError::InvalidExtraction | ReviewError::InvalidState | ReviewError::NotFound => {
            ProcessError::StructuredData
        }
        ReviewError::Persistence => ProcessError::Persistence,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use uuid::Uuid;

    use crate::{
        db::{
            mock::MockDB,
            talent::{
                ClaimedTalentItem, PublishOutcome, TalentDuplicateCandidate, TalentReviewItem,
                TalentRuntimeSettings,
            },
        },
        talent::{
            AiError, ContactCandidate, ContactType, ExtractionRequest, ImportItemStatus, SourceTrust,
            SourceType, TalentAiProvider, TalentCrypto, TalentExtraction, TalentKind,
        },
    };

    use super::{ProcessOutcome, TalentProcessor};
    use crate::talent::fragment_aad;

    struct SequenceAi {
        responses: Mutex<VecDeque<Result<TalentExtraction, AiError>>>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl TalentAiProvider for SequenceAi {
        async fn extract(&self, _request: ExtractionRequest) -> Result<TalentExtraction, AiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.responses.lock().unwrap().pop_front().unwrap()
        }
    }

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

    fn extraction() -> TalentExtraction {
        TalentExtraction {
            kind: TalentKind::Hiring,
            confidence: 0.95,
            title: Some("Cook".to_owned()),
            city: Some("Test City".to_owned()),
            salary_min: Some(6_000),
            salary_max: Some(8_000),
            roles: vec!["Cook".to_owned()],
            published_at: Some(Utc::now()),
            contacts: vec![ContactCandidate {
                contact_type: ContactType::Wechat,
                value: "candidate_2026".to_owned(),
            }],
            ..TalentExtraction::default()
        }
    }

    fn metadata(item_id: Uuid, source_trust: SourceTrust) -> TalentReviewItem {
        TalentReviewItem {
            item_id,
            batch_id: Uuid::new_v4(),
            position: 0,
            raw_fragment_ciphertext: None,
            encryption_key_version: Some(1),
            contact_candidates_ciphertext: None,
            contact_hashes: vec![],
            contact_masked: None,
            normalized_fingerprint: None,
            kind: TalentKind::Hiring,
            status: ImportItemStatus::Extracting,
            extraction: None,
            extraction_confidence: None,
            validation_errors: None,
            risk_score: None,
            risk_level: None,
            risk_reasons: None,
            duplicate_candidates: None,
            published_record_id: None,
            lock_version: 0,
            processed_at: None,
            reviewed_at: None,
            updated_at: Utc::now(),
            model_provider: Some("mock".to_owned()),
            model_name: Some("mock-model".to_owned()),
            prompt_version: Some("v1".to_owned()),
            source_id: Uuid::new_v4(),
            source_type: SourceType::WechatGroup,
            source_name: "synthetic source".to_owned(),
            source_trust_status: source_trust,
            source_auto_publish_enabled: true,
            source_confirmed_at: Some(Utc::now()),
        }
    }

    fn high_duplicate() -> TalentDuplicateCandidate {
        TalentDuplicateCandidate {
            candidate_item_id: Some(Uuid::new_v4()),
            candidate_record_id: None,
            kind: TalentKind::Hiring,
            status: "published".to_owned(),
            title: Some("Cook".to_owned()),
            roles: vec!["Cook".to_owned()],
            city: Some("Test City".to_owned()),
            district: None,
            salary_min: Some(6_000),
            salary_max: Some(8_000),
            source_id: None,
            normalized_text: "cook test city".to_owned(),
            score: 1.0,
            reasons: vec!["exact_contact_hash".to_owned()],
            high_similarity: true,
        }
    }

    fn processor_case(
        source_trust: SourceTrust,
        responses: Vec<Result<TalentExtraction, AiError>>,
        duplicates: Vec<TalentDuplicateCandidate>,
        expect_publish: bool,
    ) -> (TalentProcessor, Arc<SequenceAi>) {
        let crypto = crypto();
        let encrypted = crypto.encrypt(b"neutral synthetic input", &fragment_aad(0)).unwrap();
        let item_id = Uuid::new_v4();
        let mut db = MockDB::new();
        db.expect_claim_talent_import_item().times(1).return_once(move || {
            Ok(Some(ClaimedTalentItem {
                item_id,
                batch_id: Uuid::new_v4(),
                position: 0,
                raw_fragment_ciphertext: encrypted.bytes,
                encryption_key_version: 1,
                kind: TalentKind::Hiring,
                status: ImportItemStatus::Extracting,
            }))
        });
        db.expect_get_talent_review_item()
            .times(1)
            .return_once(move |_| Ok(Some(metadata(item_id, source_trust))));
        db.expect_find_talent_duplicate_candidates()
            .times(1)
            .return_once(move |_, _| Ok(duplicates));
        db.expect_get_talent_runtime_settings().times(1).returning(|| {
            Ok(TalentRuntimeSettings {
                auto_publish_enabled: true,
                updated_by: None,
                updated_at: Utc::now(),
            })
        });
        db.expect_complete_talent_import_item().times(1).returning(|_| Ok(()));
        if expect_publish {
            db.expect_publish_talent_item()
                .times(1)
                .withf(|publication| publication.automatic && publication.actor_user_id.is_none())
                .returning(|_| {
                    Ok(PublishOutcome {
                        record_id: Uuid::new_v4(),
                        created: true,
                    })
                });
        } else {
            db.expect_publish_talent_item().never();
        }
        let ai = Arc::new(SequenceAi {
            responses: Mutex::new(responses.into()),
            calls: AtomicUsize::new(0),
        });
        (TalentProcessor::new(Arc::new(db), crypto, ai.clone()), ai)
    }

    #[tokio::test]
    async fn auto_publishes_only_complete_trusted_low_risk_items() {
        let (processor, _) = processor_case(SourceTrust::Trusted, vec![Ok(extraction())], vec![], true);

        assert_eq!(
            processor.process_one().await.unwrap(),
            ProcessOutcome::AutoPublished
        );
    }

    #[tokio::test]
    async fn untrusted_low_risk_medium_high_and_high_duplicates_require_review() {
        for (trust, duplicates) in [
            (SourceTrust::New, vec![]),
            (SourceTrust::Watchlist, vec![]),
            (SourceTrust::Blocked, vec![]),
            (SourceTrust::Trusted, vec![high_duplicate()]),
        ] {
            let (processor, _) = processor_case(trust, vec![Ok(extraction())], duplicates, false);
            assert_eq!(
                processor.process_one().await.unwrap(),
                ProcessOutcome::NeedsReview
            );
        }
    }

    #[tokio::test]
    async fn retries_one_timeout_and_then_can_auto_publish() {
        let (processor, ai) = processor_case(
            SourceTrust::Trusted,
            vec![Err(AiError::Timeout), Ok(extraction())],
            vec![],
            true,
        );

        assert_eq!(
            processor.process_one().await.unwrap(),
            ProcessOutcome::AutoPublished
        );
        assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_second_ai_failure_is_persisted_for_review() {
        let (processor, ai) = processor_case(
            SourceTrust::Trusted,
            vec![Err(AiError::Timeout), Err(AiError::Timeout)],
            vec![],
            false,
        );

        assert_eq!(
            processor.process_one().await.unwrap(),
            ProcessOutcome::NeedsReview
        );
        assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn strict_validation_failure_is_safely_persisted_for_review() {
        let mut invalid = extraction();
        invalid.confidence = 2.0;
        let (processor, _) = processor_case(SourceTrust::Trusted, vec![Ok(invalid)], vec![], false);

        assert_eq!(
            processor.process_one().await.unwrap(),
            ProcessOutcome::NeedsReview
        );
    }

    #[tokio::test]
    async fn decrypt_failure_is_failed_with_only_a_safe_error_code() {
        let item_id = Uuid::new_v4();
        let mut db = MockDB::new();
        db.expect_claim_talent_import_item().times(1).return_once(move || {
            Ok(Some(ClaimedTalentItem {
                item_id,
                batch_id: Uuid::new_v4(),
                position: 0,
                raw_fragment_ciphertext: vec![1, 2, 3],
                encryption_key_version: 1,
                kind: TalentKind::Uncertain,
                status: ImportItemStatus::Extracting,
            }))
        });
        db.expect_fail_talent_import_item()
            .times(1)
            .withf(|failure| {
                failure.validation_errors == serde_json::json!([{"code": "decrypt_failed"}])
                    && !format!("{failure:?}").contains("[1, 2, 3]")
            })
            .returning(|_| Ok(()));
        let ai = Arc::new(SequenceAi {
            responses: Mutex::new(VecDeque::new()),
            calls: AtomicUsize::new(0),
        });
        let processor = TalentProcessor::new(Arc::new(db), crypto(), ai);

        assert_eq!(processor.process_one().await.unwrap(), ProcessOutcome::Failed);
    }
}
