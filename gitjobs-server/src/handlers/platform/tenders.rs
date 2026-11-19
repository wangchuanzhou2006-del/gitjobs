//! Authenticated tender upload, progress, retry, and cited report pages.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use askama::Template;
use axum::{
    Form,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::CACHE_CONTROL},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    auth::AuthSession,
    config::HttpServerConfig,
    db::{
        DynDB,
        tender::{NewTenderDocumentJob, NewTenderProject, TenderJobStatus, TenderJobView},
    },
    handlers::img::referer_matches_site,
    templates::{
        Config, PageId,
        platform::tenders::{
            TenderCitationView, TenderFactView, TenderJobListView, TenderListPage, TenderMaterialView,
            TenderProjectView, TenderQualificationView, TenderReportPage, TenderRiskView,
            TenderStatusComponent, TenderStatusPage, TenderUnknownView,
        },
    },
    tender::{
        TenderServices,
        model::{
            Citation, Completeness, EvidenceValue, QualificationKind, RiskSeverity, TenderAnalysisV1,
            UnknownReason,
        },
        storage::StorageError,
    },
};

pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
pub(crate) struct NewProjectForm {
    title: String,
    source_url: Option<String>,
}

fn private_response(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn enabled(tender: &TenderServices) -> bool {
    tender.storage.is_some() && tender.analysis_config_hash.is_some()
}

fn safe_referer(cfg: &HttpServerConfig, headers: &HeaderMap) -> bool {
    referer_matches_site(cfg, headers).unwrap_or(false)
}

pub(crate) async fn list(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !enabled(&tender) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Ok(projects) = db.list_tender_projects(user.user_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(jobs) = db.list_tender_jobs(user.user_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let page = TenderListPage {
        cfg: Config::from(cfg),
        page_id: PageId::Tenders,
        user: auth_session.into(),
        projects: projects
            .into_iter()
            .map(|project| TenderProjectView {
                id: project.project_id,
                title: project.title,
                source_url: project.source_url.filter(|value| safe_source_url(value)),
            })
            .collect(),
        jobs: jobs
            .into_iter()
            .map(|job| TenderJobListView {
                id: job.job_id,
                project_title: job.project_title,
                status_label: status_label(job.status),
                report_id: job.report_id,
            })
            .collect(),
    };
    match page.render() {
        Ok(html) => private_response(Html(html).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn create_project(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
    headers: HeaderMap,
    Form(form): Form<NewProjectForm>,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !enabled(&tender) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !safe_referer(&cfg, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let title = form.title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return (StatusCode::UNPROCESSABLE_ENTITY, "项目名称需在 1–200 字之间").into_response();
    }
    let source_url = form.source_url.filter(|value| !value.trim().is_empty());
    if source_url
        .as_ref()
        .is_some_and(|value| value.len() > 2000 || !safe_source_url(value))
    {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "公告链接必须为 HTTP 或 HTTPS 地址",
        )
            .into_response();
    }
    let result = db
        .create_tender_project(NewTenderProject {
            user_id: user.user_id,
            title: title.to_owned(),
            source_url,
        })
        .await;
    match result {
        Ok(_) => private_response(Redirect::to("/tenders").into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn upload(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Some(storage) = tender.storage.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !safe_referer(&cfg, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(projects) = db.list_tender_projects(user.user_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if !projects.iter().any(|project| project.project_id == project_id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(Some(mut field)) = multipart.next_field().await else {
        return (StatusCode::BAD_REQUEST, "无效的上传内容").into_response();
    };
    if field.name() != Some("file") {
        return (StatusCode::BAD_REQUEST, "请选择一个 PDF 文件").into_response();
    }
    let filename = field
        .file_name()
        .map(sanitize_filename)
        .filter(|value| !value.is_empty());

    // Pump the multipart field through a bounded pipe into the streaming store.
    // Neither the handler nor Axum's field API collects the 50 MiB body in memory.
    let (mut writer, reader) = tokio::io::duplex(64 * 1024);
    let save = storage.save(Box::pin(reader));
    let pump = async move {
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => writer.write_all(&chunk).await.map_err(|_| ())?,
                Ok(None) => break,
                Err(_) => return Err(()),
            }
        }
        Ok::<(), ()>(())
    };
    let (saved, pumped) = tokio::join!(save, pump);
    let saved = match saved {
        Ok(saved) => saved,
        Err(StorageError::FileTooLarge) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "PDF 超过 50 MiB 上限").into_response();
        }
        Err(StorageError::InvalidPdf) => {
            return (StatusCode::UNPROCESSABLE_ENTITY, "文件不是有效的 PDF").into_response();
        }
        Err(StorageError::Unavailable) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if pumped.is_err() || !matches!(multipart.next_field().await, Ok(None)) {
        let _ = storage.delete(&saved.storage_key).await;
        return (StatusCode::BAD_REQUEST, "上传内容不完整或包含额外字段").into_response();
    }
    let Some(config_hash) = tender.analysis_config_hash.as_ref() else {
        let _ = storage.delete(&saved.storage_key).await;
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let size_bytes = i64::try_from(saved.size_bytes).expect("50 MiB fits in i64");
    let outcome = db
        .create_tender_document_job(NewTenderDocumentJob {
            user_id: user.user_id,
            project_id,
            storage_key: saved.storage_key.clone(),
            sha256: saved.sha256,
            size_bytes,
            filename,
            analysis_config_hash: config_hash.clone(),
            model_provider: tender.model_provider.clone(),
            model_name: tender.model_name.clone(),
            prompt_version: Some("tender-project-info-v1".to_owned()),
        })
        .await;
    let Ok(outcome) = outcome else {
        let _ = storage.delete(&saved.storage_key).await;
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if !outcome.document_created && storage.delete(&saved.storage_key).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    private_response(Redirect::to(&format!("/tenders/jobs/{}/status", outcome.job_id)).into_response())
}

pub(crate) async fn status(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
    Path(job_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !enabled(&tender) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Ok(job) = db.get_tender_analysis_job(user.user_id, job_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(job) = job else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let component = status_component(job);
    if query.get("fragment").is_some_and(|value| value == "1") {
        return match component.render() {
            Ok(html) => private_response(Html(html).into_response()),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    }
    let page = TenderStatusPage {
        cfg: Config::from(cfg),
        page_id: PageId::Tenders,
        user: auth_session.into(),
        job_id: component.job_id,
        project_title: component.project_title,
        status_label: component.status_label,
        error_label: component.error_label,
        report_id: component.report_id,
        failed: component.failed,
        in_progress: component.in_progress,
    };
    match page.render() {
        Ok(html) => private_response(Html(html).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn retry(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
    Path(job_id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !enabled(&tender) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !safe_referer(&cfg, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(job) = db.get_tender_analysis_job(user.user_id, job_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(job) = job else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if job.status != TenderJobStatus::Failed {
        return StatusCode::CONFLICT.into_response();
    }
    if job.model_provider != tender.model_provider || job.model_name != tender.model_name {
        return (StatusCode::CONFLICT, "分析配置已变化，请重新上传文件以使用新配置").into_response();
    }
    match db.retry_tender_analysis_job(user.user_id, job_id).await {
        Ok(Some(new_id)) => {
            private_response(Redirect::to(&format!("/tenders/jobs/{new_id}/status")).into_response())
        }
        Ok(None) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn report(
    auth_session: AuthSession,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    State(tender): State<Arc<TenderServices>>,
    Path(report_id): Path<Uuid>,
) -> Response {
    let Some(user) = auth_session.user.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !enabled(&tender) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Ok(row) = db.get_tender_report(user.user_id, report_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(row) = row else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(job) = db.get_tender_analysis_job(user.user_id, row.job_id).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(job) = job.filter(|job| job.report_id == Some(report_id)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(analysis) = serde_json::from_value::<TenderAnalysisV1>(row.result) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if analysis.schema_version != "tender-analysis.v1"
        || analysis.analysis_meta.contract_version != "tender-analysis.v1"
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let page = report_page(
        Config::from(cfg),
        auth_session.into(),
        job.project_title,
        analysis,
    );
    match page.render() {
        Ok(html) => private_response(Html(html).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn safe_source_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn sanitize_filename(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect()
}

fn status_label(status: TenderJobStatus) -> &'static str {
    match status {
        TenderJobStatus::Queued => "等待处理",
        TenderJobStatus::Parsing => "正在解析",
        TenderJobStatus::Analyzing => "AI 分析中",
        TenderJobStatus::Partial => "部分完成",
        TenderJobStatus::Completed => "分析完成",
        TenderJobStatus::Failed => "分析失败",
    }
}

fn error_label(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("invalid_pdf") => Some("PDF 无法读取。请确认文件未损坏。"),
        Some("encrypted_pdf") => Some("PDF 受密码保护。请上传未加密版本。"),
        Some("too_many_pages") => Some("PDF 超过 1,000 页上限。"),
        Some("extracted_text_too_large") => Some("文件文本量超过处理上限。"),
        Some("ocr_required") => Some("这份文件主要是扫描图片，当前尚未启用文字识别。"),
        Some("analysis_unavailable") => Some("分析服务暂时不可用，可以重新分析。"),
        Some("invalid_analysis_contract") => Some("分析结果未通过原文核验，可以重新分析。"),
        Some("storage_unavailable") => Some("原文件暂时无法读取，请联系管理员。"),
        _ => None,
    }
}

fn status_component(job: TenderJobView) -> TenderStatusComponent {
    TenderStatusComponent {
        job_id: job.job_id,
        project_title: job.project_title,
        status_label: status_label(job.status),
        error_label: error_label(job.error_code.as_deref()),
        report_id: job.report_id,
        failed: job.status == TenderJobStatus::Failed,
        in_progress: matches!(
            job.status,
            TenderJobStatus::Queued | TenderJobStatus::Parsing | TenderJobStatus::Analyzing
        ),
    }
}

fn citations_view(citations: &[Citation]) -> Vec<TenderCitationView> {
    citations
        .iter()
        .map(|citation| TenderCitationView {
            page_number: citation.page_number,
            quote: citation.quote.clone(),
        })
        .collect()
}

fn fact_view(label: &'static str, evidence: &EvidenceValue) -> TenderFactView {
    TenderFactView {
        label,
        value: evidence.value.clone().unwrap_or_else(|| "待确认".to_owned()),
        confirmed: evidence.value.is_some(),
        citations: citations_view(&evidence.citations),
    }
}

fn report_page(
    cfg: Config,
    user: crate::templates::auth::User,
    project_title: String,
    report: TenderAnalysisV1,
) -> TenderReportPage {
    let facts = project_facts(&report);
    let qualifications: Vec<TenderQualificationView> = report
        .qualifications
        .iter()
        .map(|item| TenderQualificationView {
            kind: match item.kind {
                QualificationKind::HardQualification => "硬性资格",
                QualificationKind::ScoringItem => "评分项",
                QualificationKind::SubmissionMaterial => "提交材料",
                QualificationKind::ContractPerformance => "合同履约",
                QualificationKind::GeneralNote => "普通说明",
            },
            requirement: item.requirement.clone(),
            original_text: item.original_text.clone(),
            citations: citations_view(&item.citations),
        })
        .collect();
    let risks: Vec<TenderRiskView> = report
        .risks
        .iter()
        .map(|item| TenderRiskView {
            severity: match item.severity {
                RiskSeverity::Low => "低",
                RiskSeverity::Medium => "中",
                RiskSeverity::High => "高",
            },
            fact: item.fact.clone(),
            judgment: item.risk_judgment.clone(),
            impact: item.impact.clone(),
            suggestion: item.suggestion.clone(),
            citations: citations_view(&item.citations),
        })
        .collect();
    let materials: Vec<TenderMaterialView> = report
        .required_materials
        .iter()
        .map(|item| TenderMaterialView {
            name: item.name.clone(),
            description: item.description.clone(),
            source_requirement: item.source_requirement.clone(),
            citations: citations_view(&item.citations),
        })
        .collect();
    let unknowns: Vec<TenderUnknownView> = report
        .unknowns
        .iter()
        .map(|item| TenderUnknownView {
            field_name: item.field_name.clone(),
            detail: item.detail.clone(),
            reason: match item.reason {
                UnknownReason::Missing => "缺失",
                UnknownReason::Conflict => "原文冲突",
                UnknownReason::LowConfidence => "置信度不足",
                UnknownReason::Unsupported => "缺少依据",
            },
            citations: citations_view(&item.citations),
        })
        .collect();
    let mut seen = HashSet::new();
    let sources = facts
        .iter()
        .flat_map(|item| &item.citations)
        .chain(qualifications.iter().flat_map(|item| &item.citations))
        .chain(risks.iter().flat_map(|item| &item.citations))
        .chain(materials.iter().flat_map(|item| &item.citations))
        .chain(unknowns.iter().flat_map(|item| &item.citations))
        .filter(|citation| seen.insert((citation.page_number, citation.quote.clone())))
        .map(|citation| TenderCitationView {
            page_number: citation.page_number,
            quote: citation.quote.clone(),
        })
        .collect();
    TenderReportPage {
        cfg,
        page_id: PageId::Tenders,
        user,
        project_title,
        report_status: match report.analysis_meta.completeness {
            Completeness::Complete => "分析完成",
            Completeness::Partial => "部分完成 · 请人工核对",
        },
        model_provider: report.analysis_meta.model_provider,
        model_name: report.analysis_meta.model_name,
        prompt_versions: vec![
            report.analysis_meta.prompt_versions.project_info,
            report.analysis_meta.prompt_versions.qualifications,
            report.analysis_meta.prompt_versions.risks,
            report.analysis_meta.prompt_versions.materials,
            report.analysis_meta.prompt_versions.review,
        ],
        facts,
        qualifications,
        risks,
        materials,
        unknowns,
        sources,
    }
}

fn project_facts(report: &TenderAnalysisV1) -> Vec<TenderFactView> {
    vec![
        fact_view("项目名称", &report.project_info.project_name),
        fact_view("采购单位", &report.project_info.procurement_entity),
        fact_view("预算金额", &report.project_info.budget_amount),
        fact_view("预算币种", &report.project_info.budget_currency),
        fact_view("服务期限", &report.project_info.service_period),
        fact_view("投标截止时间", &report.project_info.bid_deadline),
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header::COOKIE},
    };
    use axum_login::tower_sessions::session;
    use chrono::Utc;
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;
    use tower::ServiceExt;

    use crate::{
        db::{
            mock::MockDB,
            tender::{CreateJobOutcome, TenderProjectRow},
        },
        handlers::tests::{TestRouterBuilder, sample_auth_user, sample_session_record, test_http_server_cfg},
        notifications::MockNotificationsManager,
        tender::storage::{PdfStream, StoredTenderDocument, TenderDocumentStore},
    };

    use super::*;

    struct MemoryStore {
        bytes: Mutex<Vec<u8>>,
        deleted: AtomicUsize,
    }

    #[async_trait]
    impl TenderDocumentStore for MemoryStore {
        async fn save(&self, mut stream: PdfStream) -> Result<StoredTenderDocument, StorageError> {
            let mut bytes = Vec::new();
            stream
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| StorageError::Unavailable)?;
            if !bytes.starts_with(b"%PDF-") {
                return Err(StorageError::InvalidPdf);
            }
            *self.bytes.lock().unwrap() = bytes.clone();
            Ok(StoredTenderDocument {
                storage_key: Uuid::new_v4().to_string(),
                sha256: crate::tender::hex_bytes(&Sha256::digest(&bytes)),
                size_bytes: bytes.len() as u64,
            })
        }
        async fn open(&self, _key: &str) -> Result<Vec<u8>, StorageError> {
            Ok(self.bytes.lock().unwrap().clone())
        }
        async fn delete(&self, _key: &str) -> Result<(), StorageError> {
            self.deleted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn test_tender(storage: Arc<MemoryStore>) -> Arc<TenderServices> {
        Arc::new(TenderServices {
            storage: Some(storage),
            processor: None,
            analysis_config_hash: Some("fixture-config-hash".to_owned()),
            model_provider: Some("fixture".to_owned()),
            model_name: Some("fixture-model".to_owned()),
        })
    }

    fn user_session(db: &mut MockDB) -> (session::Id, Uuid) {
        let id = session::Id::default();
        let user_id = Uuid::new_v4();
        let record = sample_session_record(id, user_id, "hash", None);
        let user = sample_auth_user(user_id, "hash");
        db.expect_get_session().returning(move |_| Ok(Some(record.clone())));
        db.expect_get_user_by_id().returning(move |_| Ok(Some(user.clone())));
        (id, user_id)
    }

    fn project(id: Uuid) -> TenderProjectRow {
        TenderProjectRow {
            project_id: id,
            title: "虚构食堂项目".to_owned(),
            source_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            latest_job_id: None,
            latest_job_status: None,
            latest_report_id: None,
        }
    }

    #[tokio::test]
    async fn anonymous_routes_redirect_before_reading_upload() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/tenders/{}/documents", Uuid::new_v4()))
                    .header("content-type", "multipart/form-data; boundary=b")
                    .body(Body::from("PRIVATE_SENTINEL_NEVER_READ"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(!body.windows(16).any(|part| part == b"PRIVATE_SENTINEL"));
    }

    #[tokio::test]
    async fn authenticated_upload_creates_a_private_job() {
        let project_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let mut db = MockDB::new();
        let (session_id, user_id) = user_session(&mut db);
        db.expect_list_tender_projects()
            .withf(move |id| *id == user_id)
            .times(1)
            .returning(move |_| Ok(vec![project(project_id)]));
        db.expect_create_tender_document_job()
            .times(1)
            .withf(move |input| {
                input.user_id == user_id
                    && input.project_id == project_id
                    && input.analysis_config_hash == "fixture-config-hash"
                    && input.sha256.len() == 64
                    && input.filename.as_deref() == Some("sample.pdf")
            })
            .returning(move |_| {
                Ok(CreateJobOutcome {
                    job_id,
                    created: true,
                    document_created: true,
                })
            });
        let storage = Arc::new(MemoryStore {
            bytes: Mutex::new(Vec::new()),
            deleted: AtomicUsize::new(0),
        });
        let tender = test_tender(storage.clone());
        let mut cfg = test_http_server_cfg();
        cfg.tender.enabled = true;
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .with_cfg(cfg)
            .with_tender_services(tender)
            .build()
            .await;
        let body = b"--b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-fictional\r\n--b--\r\n";
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            router.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/tenders/{project_id}/documents"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("referer", "http://localhost:9000/tenders")
                    .header("content-type", "multipart/form-data; boundary=b")
                    .body(Body::from(body.as_slice()))
                    .unwrap(),
            ),
        )
        .await
        .expect("upload must finish after the multipart stream ends")
        .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()["location"],
            format!("/tenders/jobs/{job_id}/status")
        );
        assert_eq!(*storage.bytes.lock().unwrap(), b"%PDF-fictional");
        assert_eq!(storage.deleted.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn report_escapes_untrusted_model_text() {
        let mut report: TenderAnalysisV1 = serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/tender-analysis.v1.json"
        ))
        .unwrap();
        report.project_info.project_name.value = Some("<script>alert('fictional')</script>".to_owned());
        let page = report_page(
            Config::from(test_http_server_cfg()),
            crate::templates::auth::User::default(),
            "合成项目".to_owned(),
            report,
        );
        let html = page.render().unwrap();
        assert!(!html.contains("<script>alert('fictional')</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("查看第 1 页原文"));
        assert!(html.contains("待人工确认"));
    }

    #[test]
    fn filenames_and_source_links_fail_closed() {
        assert_eq!(sanitize_filename("../../outside\\sample.pdf"), "sample.pdf");
        assert!(!safe_source_url("javascript:alert(1)"));
        assert!(!safe_source_url("https://user:pass@example.invalid/x"));
        assert!(safe_source_url("https://example.invalid/tender"));
    }
}
