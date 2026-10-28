//! Persistence boundary for tender projects, documents, analysis jobs, and reports.

use std::fmt;

use anyhow::{Result, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::types::Json;
use tracing::{instrument, trace};
use uuid::Uuid;

use crate::PgDB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TenderJobStatus {
    Queued,
    Parsing,
    Analyzing,
    Partial,
    Completed,
    Failed,
}

impl TenderJobStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Parsing => "parsing",
            Self::Analyzing => "analyzing",
            Self::Partial => "partial",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for TenderJobStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "parsing" => Ok(Self::Parsing),
            "analyzing" => Ok(Self::Analyzing),
            "partial" => Ok(Self::Partial),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => bail!("invalid tender analysis job status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NewTenderProject {
    pub user_id: Uuid,
    pub title: String,
    pub source_url: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NewTenderDocumentJob {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub storage_key: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub filename: Option<String>,
    pub analysis_config_hash: String,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
}

impl fmt::Debug for NewTenderDocumentJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewTenderDocumentJob")
            .field("user_id", &self.user_id)
            .field("project_id", &self.project_id)
            .field("storage_key", &"[REDACTED]")
            .field("sha256", &self.sha256)
            .field("size_bytes", &self.size_bytes)
            .field("filename", &self.filename)
            .field("analysis_config_hash", &self.analysis_config_hash)
            .field("model_provider", &self.model_provider)
            .field("model_name", &self.model_name)
            .field("prompt_version", &self.prompt_version)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CreateJobOutcome {
    pub job_id: Uuid,
    pub created: bool,
    pub document_created: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ClaimedTenderJob {
    pub job_id: Uuid,
    pub document_id: Uuid,
    pub status: TenderJobStatus,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub claim_expires_at: DateTime<Utc>,
    pub attempt_count: i32,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
    pub storage_key: String,
    pub sha256: String,
}

impl fmt::Debug for ClaimedTenderJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaimedTenderJob")
            .field("job_id", &self.job_id)
            .field("document_id", &self.document_id)
            .field("status", &self.status)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("claim_expires_at", &self.claim_expires_at)
            .field("attempt_count", &self.attempt_count)
            .field("model_provider", &self.model_provider)
            .field("model_name", &self.model_name)
            .field("prompt_version", &self.prompt_version)
            .field("storage_key", &"[REDACTED]")
            .field("sha256", &self.sha256)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TenderJobTransition {
    pub job_id: Uuid,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub next_status: TenderJobStatus,
}

impl fmt::Debug for TenderJobTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TenderJobTransition")
            .field("job_id", &self.job_id)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("next_status", &self.next_status)
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct CompletedTenderJob {
    pub job_id: Uuid,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub status: TenderJobStatus,
    pub result: Value,
}

impl fmt::Debug for CompletedTenderJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedTenderJob")
            .field("job_id", &self.job_id)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("status", &self.status)
            .field("result", &"[REDACTED SAFE JSON]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FailedTenderJob {
    pub job_id: Uuid,
    pub claim_token: Uuid,
    pub claim_version: i64,
    pub error_code: String,
}

impl fmt::Debug for FailedTenderJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FailedTenderJob")
            .field("job_id", &self.job_id)
            .field("claim_token", &"[REDACTED]")
            .field("claim_version", &self.claim_version)
            .field("error_code", &self.error_code)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TenderJobView {
    pub job_id: Uuid,
    pub document_id: Uuid,
    pub project_id: Uuid,
    pub project_title: String,
    pub status: TenderJobStatus,
    pub error_code: Option<String>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub prompt_version: Option<String>,
    pub attempt_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TenderProjectRow {
    pub project_id: Uuid,
    pub title: String,
    pub source_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TenderReportRow {
    pub report_id: Uuid,
    pub job_id: Uuid,
    pub document_id: Uuid,
    pub result: Value,
    pub contract_version: String,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for TenderReportRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TenderReportRow")
            .field("report_id", &self.report_id)
            .field("job_id", &self.job_id)
            .field("document_id", &self.document_id)
            .field("result", &"[REDACTED SAFE JSON]")
            .field("contract_version", &self.contract_version)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Deserialize)]
struct ClaimedTenderJobWire {
    job_id: Uuid,
    document_id: Uuid,
    status: TenderJobStatus,
    claim_token: Uuid,
    claim_version: i64,
    claim_expires_at: DateTime<Utc>,
    attempt_count: i32,
    model_provider: Option<String>,
    model_name: Option<String>,
    prompt_version: Option<String>,
    storage_key: String,
    sha256: String,
}

impl ClaimedTenderJobWire {
    fn try_into_claimed(self) -> Result<ClaimedTenderJob> {
        if !matches!(self.status, TenderJobStatus::Parsing | TenderJobStatus::Analyzing) {
            bail!("claimed tender analysis job is not in-flight");
        }
        Ok(ClaimedTenderJob {
            job_id: self.job_id,
            document_id: self.document_id,
            status: self.status,
            claim_token: self.claim_token,
            claim_version: self.claim_version,
            claim_expires_at: self.claim_expires_at,
            attempt_count: self.attempt_count,
            model_provider: self.model_provider,
            model_name: self.model_name,
            prompt_version: self.prompt_version,
            storage_key: self.storage_key,
            sha256: self.sha256,
        })
    }
}

#[async_trait]
pub(crate) trait DBTender {
    async fn create_tender_project(&self, input: NewTenderProject) -> Result<Uuid>;
    async fn create_tender_document_job(&self, input: NewTenderDocumentJob) -> Result<CreateJobOutcome>;
    async fn claim_tender_analysis_job(&self) -> Result<Option<ClaimedTenderJob>>;
    async fn transition_tender_analysis_job(&self, input: TenderJobTransition) -> Result<()>;
    async fn complete_tender_analysis_job(&self, input: CompletedTenderJob) -> Result<Uuid>;
    async fn fail_tender_analysis_job(&self, input: FailedTenderJob) -> Result<()>;
    async fn get_tender_analysis_job(&self, user_id: Uuid, job_id: Uuid) -> Result<Option<TenderJobView>>;
    async fn list_tender_projects(&self, user_id: Uuid) -> Result<Vec<TenderProjectRow>>;
    async fn get_tender_report(&self, user_id: Uuid, report_id: Uuid) -> Result<Option<TenderReportRow>>;
}

#[async_trait]
impl DBTender for PgDB {
    #[instrument(skip(self, input), err)]
    async fn create_tender_project(&self, input: NewTenderProject) -> Result<Uuid> {
        trace!("db: create tender project");
        let payload = serde_json::to_value(&input)?;
        let db = self.pool.get().await?;
        let row = db
            .query_one("select create_tender_project($1::jsonb);", &[&Json(&payload)])
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self, input), err)]
    async fn create_tender_document_job(&self, input: NewTenderDocumentJob) -> Result<CreateJobOutcome> {
        trace!("db: create tender document job");
        let payload = serde_json::to_value(&input)?;
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select create_tender_document_job($1::jsonb)::text;",
                &[&Json(&payload)],
            )
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn claim_tender_analysis_job(&self) -> Result<Option<ClaimedTenderJob>> {
        trace!("db: claim tender analysis job");
        let db = self.pool.get().await?;
        let row = db.query_one("select claim_tender_analysis_job()::text;", &[]).await?;
        row.get::<_, Option<String>>(0)
            .map(|json| serde_json::from_str::<ClaimedTenderJobWire>(&json))
            .transpose()?
            .map(ClaimedTenderJobWire::try_into_claimed)
            .transpose()
    }

    #[instrument(skip(self, input), err)]
    async fn transition_tender_analysis_job(&self, input: TenderJobTransition) -> Result<()> {
        trace!("db: transition tender analysis job");
        let payload = serde_json::to_value(&input)?;
        let db = self.pool.get().await?;
        db.execute(
            "select transition_tender_analysis_job($1::jsonb);",
            &[&Json(&payload)],
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self, input), err)]
    async fn complete_tender_analysis_job(&self, input: CompletedTenderJob) -> Result<Uuid> {
        trace!("db: complete tender analysis job");
        let payload = serde_json::to_value(&input)?;
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select complete_tender_analysis_job($1::jsonb);",
                &[&Json(&payload)],
            )
            .await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self, input), err)]
    async fn fail_tender_analysis_job(&self, input: FailedTenderJob) -> Result<()> {
        trace!("db: fail tender analysis job");
        let payload = serde_json::to_value(&input)?;
        let db = self.pool.get().await?;
        db.execute("select fail_tender_analysis_job($1::jsonb);", &[&Json(&payload)])
            .await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    async fn get_tender_analysis_job(&self, user_id: Uuid, job_id: Uuid) -> Result<Option<TenderJobView>> {
        trace!("db: get tender analysis job");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select get_tender_analysis_job($1::uuid, $2::uuid)::text;",
                &[&user_id, &job_id],
            )
            .await?;
        row.get::<_, Option<String>>(0)
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(Into::into)
    }

    #[instrument(skip(self), err)]
    async fn list_tender_projects(&self, user_id: Uuid) -> Result<Vec<TenderProjectRow>> {
        trace!("db: list tender projects");
        let db = self.pool.get().await?;
        let row = db
            .query_one("select list_tender_projects($1::uuid)::text;", &[&user_id])
            .await?;
        Ok(serde_json::from_str(&row.get::<_, String>(0))?)
    }

    #[instrument(skip(self), err)]
    async fn get_tender_report(&self, user_id: Uuid, report_id: Uuid) -> Result<Option<TenderReportRow>> {
        trace!("db: get tender report");
        let db = self.pool.get().await?;
        let row = db
            .query_one(
                "select get_tender_report($1::uuid, $2::uuid)::text;",
                &[&user_id, &report_id],
            )
            .await?;
        row.get::<_, Option<String>>(0)
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        ClaimedTenderJob, CompletedTenderJob, NewTenderDocumentJob, TenderJobStatus, TenderReportRow,
    };

    #[test]
    fn job_status_round_trips_through_names() {
        for (status, name) in [
            (TenderJobStatus::Queued, "queued"),
            (TenderJobStatus::Parsing, "parsing"),
            (TenderJobStatus::Analyzing, "analyzing"),
            (TenderJobStatus::Partial, "partial"),
            (TenderJobStatus::Completed, "completed"),
            (TenderJobStatus::Failed, "failed"),
        ] {
            assert_eq!(status.as_str(), name);
            assert_eq!(TenderJobStatus::try_from(name).unwrap(), status);
        }
        assert!(TenderJobStatus::try_from("invalid").is_err());
    }

    #[test]
    fn document_job_debug_redacts_storage_key() {
        let input = NewTenderDocumentJob {
            user_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            storage_key: "docs/secret/path.pdf".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 10,
            filename: Some("tender.pdf".to_owned()),
            analysis_config_hash: "config-v1".to_owned(),
            model_provider: Some("openai".to_owned()),
            model_name: Some("gpt-4o".to_owned()),
            prompt_version: Some("tender-project-info-v1".to_owned()),
        };

        let debug = format!("{input:?}");
        assert!(!debug.contains("secret/path"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn completion_debug_redacts_result_json() {
        let completed = CompletedTenderJob {
            job_id: Uuid::new_v4(),
            claim_token: Uuid::new_v4(),
            claim_version: 1,
            status: TenderJobStatus::Completed,
            result: json!({"project_info": {"project_name": {"value": "private-title"}}}),
        };

        let debug = format!("{completed:?}");
        assert!(!debug.contains("private-title"));
        assert!(debug.contains("[REDACTED SAFE JSON]"));
    }

    #[test]
    fn report_debug_redacts_result_json() {
        let report = TenderReportRow {
            report_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            result: json!({"project_info": {"project_name": {"value": "private-title"}}}),
            contract_version: "tender-analysis.v1".to_owned(),
            created_at: chrono::Utc::now(),
        };

        let debug = format!("{report:?}");
        assert!(!debug.contains("private-title"));
        assert!(debug.contains("[REDACTED SAFE JSON]"));
    }

    #[test]
    fn claimed_job_debug_redacts_storage_key_and_token() {
        let claimed = ClaimedTenderJob {
            job_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            status: TenderJobStatus::Parsing,
            claim_token: Uuid::new_v4(),
            claim_version: 1,
            claim_expires_at: chrono::Utc::now(),
            attempt_count: 1,
            model_provider: None,
            model_name: None,
            prompt_version: None,
            storage_key: "docs/secret/path.pdf".to_owned(),
            sha256: "a".repeat(64),
        };

        let debug = format!("{claimed:?}");
        assert!(!debug.contains("secret/path"));
        assert!(!debug.contains(&claimed.claim_token.to_string()));
        assert!(debug.contains("[REDACTED]"));
    }
}
