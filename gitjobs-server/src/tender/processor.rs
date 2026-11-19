//! Durable one-claim tender processing with database fencing.

use std::{sync::Arc, time::Duration};

use chrono::{Duration as ChronoDuration, Utc};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::db::{
    DynDB,
    tender::{ClaimedTenderJob, CompletedTenderJob, FailedTenderJob, TenderJobStatus, TenderJobTransition},
};

use super::{
    client::{ClientError, TenderAnalysisClient},
    model::{Completeness, ParsedDocument, TenderAnalysisV1},
    storage::{StorageError, TenderDocumentStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TenderProcessOutcome {
    NoWork,
    Completed(Uuid),
    Failed,
    Deferred,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TenderProcessError {
    #[error("tender_repository_unavailable")]
    RepositoryUnavailable,
    #[error("tender_stale_claim")]
    StaleClaim,
}

pub(crate) struct TenderProcessor {
    db: DynDB,
    storage: Arc<dyn TenderDocumentStore>,
    client: Arc<dyn TenderAnalysisClient>,
    request_timeout: Duration,
    model_provider: String,
    model_name: String,
}

impl TenderProcessor {
    pub(crate) fn new(
        db: DynDB,
        storage: Arc<dyn TenderDocumentStore>,
        client: Arc<dyn TenderAnalysisClient>,
        request_timeout: Duration,
        model_provider: String,
        model_name: String,
    ) -> Self {
        Self {
            db,
            storage,
            client,
            request_timeout,
            model_provider,
            model_name,
        }
    }

    /// A retryable failure leaves the fenced claim in-flight; the next worker
    /// can re-claim it only after its lease expires. Three claims is the cap.
    pub(crate) async fn process_one(&self) -> Result<TenderProcessOutcome, TenderProcessError> {
        let Some(claim) = self
            .db
            .claim_tender_analysis_job()
            .await
            .map_err(|_| TenderProcessError::RepositoryUnavailable)?
        else {
            return Ok(TenderProcessOutcome::NoWork);
        };
        if claim.attempt_count > 3 {
            return self.handle_error(&claim, "analysis_unavailable", false).await;
        }
        if claim
            .model_provider
            .as_ref()
            .is_some_and(|value| value != &self.model_provider)
            || claim
                .model_name
                .as_ref()
                .is_some_and(|value| value != &self.model_name)
        {
            return self.handle_error(&claim, "analysis_unavailable", false).await;
        }
        let pdf = match self.storage.open(&claim.storage_key).await {
            Ok(pdf) => pdf,
            Err(error) => return self.handle_error(&claim, storage_error_code(&error), false).await,
        };
        if super::hex_bytes(&Sha256::digest(&pdf)) != claim.sha256 {
            return self.handle_error(&claim, "storage_unavailable", false).await;
        }
        let document = match self.parse_with_one_retry(&claim, &pdf).await {
            Ok(document) => document,
            Err(error) => {
                return self
                    .handle_error(&claim, client_error_code(error), error.is_retryable())
                    .await;
            }
        };
        if claim.status == TenderJobStatus::Parsing {
            self.db
                .transition_tender_analysis_job(TenderJobTransition {
                    job_id: claim.job_id,
                    claim_token: claim.claim_token,
                    claim_version: claim.claim_version,
                    next_status: TenderJobStatus::Analyzing,
                })
                .await
                .map_err(|error| classify_db_error(&error))?;
        }
        let report = match self.analyze_with_one_retry(&claim, &document).await {
            Ok(report) => report,
            Err(error) => {
                return self
                    .handle_error(&claim, client_error_code(error), error.is_retryable())
                    .await;
            }
        };
        if report.validate_against(&document).is_err()
            || report.analysis_meta.model_provider != self.model_provider
            || report.analysis_meta.model_name != self.model_name
        {
            return self.handle_error(&claim, "invalid_analysis_contract", false).await;
        }
        let status = match report.analysis_meta.completeness {
            Completeness::Complete => TenderJobStatus::Completed,
            Completeness::Partial => TenderJobStatus::Partial,
        };
        let Ok(result) = serde_json::to_value(&report) else {
            return self.handle_error(&claim, "invalid_analysis_contract", false).await;
        };
        let report_id = self
            .db
            .complete_tender_analysis_job(CompletedTenderJob {
                job_id: claim.job_id,
                claim_token: claim.claim_token,
                claim_version: claim.claim_version,
                status,
                result,
            })
            .await
            .map_err(|error| classify_db_error(&error))?;
        Ok(TenderProcessOutcome::Completed(report_id))
    }

    async fn parse_with_one_retry(
        &self,
        claim: &ClaimedTenderJob,
        pdf: &[u8],
    ) -> Result<ParsedDocument, ClientError> {
        match self.client.parse(pdf).await {
            Err(error) if error.is_retryable() && self.can_retry_once(claim) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                self.client.parse(pdf).await
            }
            result => result,
        }
    }

    async fn analyze_with_one_retry(
        &self,
        claim: &ClaimedTenderJob,
        document: &ParsedDocument,
    ) -> Result<TenderAnalysisV1, ClientError> {
        match self.client.analyze(claim.job_id, document).await {
            Err(error) if error.is_retryable() && self.can_retry_once(claim) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                self.client.analyze(claim.job_id, document).await
            }
            result => result,
        }
    }

    fn can_retry_once(&self, claim: &ClaimedTenderJob) -> bool {
        if claim.attempt_count != 1 {
            return false;
        }
        let reserve = self
            .request_timeout
            .checked_add(Duration::from_secs(15))
            .unwrap_or(self.request_timeout);
        ChronoDuration::from_std(reserve)
            .is_ok_and(|needed| claim.claim_expires_at.signed_duration_since(Utc::now()) > needed)
    }

    async fn handle_error(
        &self,
        claim: &ClaimedTenderJob,
        code: &'static str,
        retryable: bool,
    ) -> Result<TenderProcessOutcome, TenderProcessError> {
        if retryable && claim.attempt_count < 3 {
            return Ok(TenderProcessOutcome::Deferred);
        }
        self.db
            .fail_tender_analysis_job(FailedTenderJob {
                job_id: claim.job_id,
                claim_token: claim.claim_token,
                claim_version: claim.claim_version,
                error_code: code.to_owned(),
            })
            .await
            .map_err(|error| classify_db_error(&error))?;
        Ok(TenderProcessOutcome::Failed)
    }
}

fn storage_error_code(error: &StorageError) -> &'static str {
    match error {
        StorageError::InvalidPdf => "invalid_pdf",
        StorageError::FileTooLarge | StorageError::Unavailable => "storage_unavailable",
    }
}

fn client_error_code(error: ClientError) -> &'static str {
    match error {
        ClientError::InvalidPdf => "invalid_pdf",
        ClientError::EncryptedPdf => "encrypted_pdf",
        ClientError::TooManyPages => "too_many_pages",
        ClientError::ExtractedTextTooLarge => "extracted_text_too_large",
        ClientError::OcrRequired => "ocr_required",
        ClientError::InvalidContract => "invalid_analysis_contract",
        ClientError::Unavailable | ClientError::Unauthorized => "analysis_unavailable",
    }
}

fn classify_db_error(error: &anyhow::Error) -> TenderProcessError {
    if error
        .chain()
        .filter_map(|source| source.downcast_ref::<tokio_postgres::Error>())
        .any(|db_error| db_error.code().is_some_and(|code| code.code() == "40001"))
    {
        TenderProcessError::StaleClaim
    } else {
        TenderProcessError::RepositoryUnavailable
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;

    use crate::db::mock::MockDB;

    use super::*;
    use crate::tender::{
        model::{DocumentChunk, ParsedPage},
        storage::{PdfStream, StoredTenderDocument},
    };

    const PDF: &[u8] = b"%PDF-fictional";

    struct MemoryStore;

    #[async_trait]
    impl TenderDocumentStore for MemoryStore {
        async fn save(&self, _stream: PdfStream) -> Result<StoredTenderDocument, StorageError> {
            unreachable!()
        }
        async fn open(&self, _key: &str) -> Result<Vec<u8>, StorageError> {
            Ok(PDF.to_vec())
        }
        async fn delete(&self, _key: &str) -> Result<(), StorageError> {
            unreachable!()
        }
    }

    struct FakeClient {
        calls: Mutex<Vec<&'static str>>,
        parse_failures: AtomicUsize,
        invalid_quote: bool,
    }

    impl FakeClient {
        fn new(parse_failures: usize, invalid_quote: bool) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                parse_failures: AtomicUsize::new(parse_failures),
                invalid_quote,
            }
        }
    }

    #[async_trait]
    impl TenderAnalysisClient for FakeClient {
        async fn parse(&self, _pdf: &[u8]) -> Result<ParsedDocument, ClientError> {
            self.calls.lock().unwrap().push("parse");
            if self
                .parse_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(ClientError::Unavailable);
            }
            Ok(ParsedDocument {
                page_count: 1,
                pages: vec![ParsedPage {
                    page_number: 1,
                    text: "synthetic cited clause".to_owned(),
                    chunks: vec![DocumentChunk {
                        chunk_id: "page-1-chunk-0".to_owned(),
                        page_number: 1,
                        text: "synthetic cited clause".to_owned(),
                    }],
                }],
            })
        }

        async fn analyze(
            &self,
            _job_id: Uuid,
            _document: &ParsedDocument,
        ) -> Result<TenderAnalysisV1, ClientError> {
            self.calls.lock().unwrap().push("analyze");
            let evidence = serde_json::json!({"value": null, "confidence": 0.0, "requires_review": true, "citations": []});
            let mut report = serde_json::json!({
                "schema_version": "tender-analysis.v1",
                "project_info": {
                    "project_name": evidence, "procurement_entity": evidence, "budget_amount": evidence,
                    "budget_currency": evidence, "service_period": evidence, "bid_deadline": evidence,
                },
                "qualifications": [], "risks": [], "required_materials": [], "unknowns": [],
                "analysis_meta": {
                    "model_provider": "fixture", "model_name": "fixture-model",
                    "prompt_versions": {
                        "project_info": "tender-project-info-v1", "qualifications": "tender-qualifications-v1",
                        "risks": "tender-risks-v1", "materials": "tender-materials-v1", "review": "tender-review-v1",
                    },
                    "contract_version": "tender-analysis.v1", "completeness": "partial",
                },
            });
            if self.invalid_quote {
                report["project_info"]["project_name"] = serde_json::json!({
                    "value": "fabricated", "confidence": 0.9, "requires_review": false,
                    "citations": [{"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "fabricated quote"}],
                });
            }
            Ok(serde_json::from_value(report).unwrap())
        }
    }

    fn claim(attempt_count: i32, status: TenderJobStatus) -> ClaimedTenderJob {
        ClaimedTenderJob {
            job_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            status,
            claim_token: Uuid::new_v4(),
            claim_version: 1,
            claim_expires_at: Utc::now() + ChronoDuration::minutes(10),
            attempt_count,
            model_provider: None,
            model_name: None,
            prompt_version: None,
            storage_key: Uuid::new_v4().to_string(),
            sha256: crate::tender::hex_bytes(&Sha256::digest(PDF)),
        }
    }

    #[tokio::test]
    async fn parses_analyzes_and_completes_only_after_validation() {
        let claim = claim(1, TenderJobStatus::Parsing);
        let report_id = Uuid::new_v4();
        let mut db = MockDB::new();
        db.expect_claim_tender_analysis_job()
            .times(1)
            .return_once(move || Ok(Some(claim)));
        db.expect_transition_tender_analysis_job()
            .times(1)
            .withf(|input| input.next_status == TenderJobStatus::Analyzing)
            .returning(|_| Ok(()));
        db.expect_complete_tender_analysis_job()
            .times(1)
            .withf(|input| input.status == TenderJobStatus::Partial)
            .returning(move |_| Ok(report_id));
        db.expect_fail_tender_analysis_job().never();
        let client = Arc::new(FakeClient::new(0, false));
        let processor = TenderProcessor::new(
            Arc::new(db),
            Arc::new(MemoryStore),
            client.clone(),
            Duration::from_secs(3),
            "fixture".to_owned(),
            "fixture-model".to_owned(),
        );
        assert_eq!(
            processor.process_one().await.unwrap(),
            TenderProcessOutcome::Completed(report_id)
        );
        assert_eq!(*client.calls.lock().unwrap(), ["parse", "analyze"]);
    }

    #[tokio::test]
    async fn retryable_transport_failure_gets_one_immediate_retry_then_defers() {
        let claim = claim(1, TenderJobStatus::Parsing);
        let mut db = MockDB::new();
        db.expect_claim_tender_analysis_job()
            .times(1)
            .return_once(move || Ok(Some(claim)));
        db.expect_transition_tender_analysis_job().never();
        db.expect_fail_tender_analysis_job().never();
        let client = Arc::new(FakeClient::new(2, false));
        let processor = TenderProcessor::new(
            Arc::new(db),
            Arc::new(MemoryStore),
            client.clone(),
            Duration::from_secs(3),
            "fixture".to_owned(),
            "fixture-model".to_owned(),
        );
        assert_eq!(
            processor.process_one().await.unwrap(),
            TenderProcessOutcome::Deferred
        );
        assert_eq!(*client.calls.lock().unwrap(), ["parse", "parse"]);
    }

    #[tokio::test]
    async fn third_claim_fails_permanently_after_transport_error() {
        let claim = claim(3, TenderJobStatus::Parsing);
        let mut db = MockDB::new();
        db.expect_claim_tender_analysis_job()
            .times(1)
            .return_once(move || Ok(Some(claim)));
        db.expect_fail_tender_analysis_job()
            .times(1)
            .withf(|input| input.error_code == "analysis_unavailable")
            .returning(|_| Ok(()));
        let client = Arc::new(FakeClient::new(1, false));
        let processor = TenderProcessor::new(
            Arc::new(db),
            Arc::new(MemoryStore),
            client.clone(),
            Duration::from_secs(3),
            "fixture".to_owned(),
            "fixture-model".to_owned(),
        );
        assert_eq!(
            processor.process_one().await.unwrap(),
            TenderProcessOutcome::Failed
        );
        assert_eq!(*client.calls.lock().unwrap(), ["parse"]);
    }

    #[tokio::test]
    async fn invented_quote_never_reaches_report_storage() {
        let claim = claim(1, TenderJobStatus::Analyzing);
        let mut db = MockDB::new();
        db.expect_claim_tender_analysis_job()
            .times(1)
            .return_once(move || Ok(Some(claim)));
        db.expect_transition_tender_analysis_job().never();
        db.expect_complete_tender_analysis_job().never();
        db.expect_fail_tender_analysis_job()
            .times(1)
            .withf(|input| input.error_code == "invalid_analysis_contract")
            .returning(|_| Ok(()));
        let client = Arc::new(FakeClient::new(0, true));
        let processor = TenderProcessor::new(
            Arc::new(db),
            Arc::new(MemoryStore),
            client,
            Duration::from_secs(3),
            "fixture".to_owned(),
            "fixture-model".to_owned(),
        );
        assert_eq!(
            processor.process_one().await.unwrap(),
            TenderProcessOutcome::Failed
        );
    }
}
