//! Authenticated tender pages; all model strings are Askama-escaped.

use askama::Template;
use uuid::Uuid;

use crate::templates::{Config, PageId, auth::User};

#[derive(Template)]
#[template(path = "platform/tenders/list.html")]
pub(crate) struct TenderListPage {
    pub cfg: Config,
    pub page_id: PageId,
    #[allow(dead_code)]
    pub user: User,
    pub projects: Vec<TenderProjectView>,
    pub jobs: Vec<TenderJobListView>,
}

pub(crate) struct TenderProjectView {
    pub id: Uuid,
    pub title: String,
    pub source_url: Option<String>,
}

pub(crate) struct TenderJobListView {
    pub id: Uuid,
    pub project_title: String,
    pub status_label: &'static str,
    pub report_id: Option<Uuid>,
}

#[derive(Clone, Template)]
#[template(path = "platform/tenders/components/status.html")]
pub(crate) struct TenderStatusComponent {
    pub job_id: Uuid,
    pub project_title: String,
    pub status_label: &'static str,
    pub error_label: Option<&'static str>,
    pub report_id: Option<Uuid>,
    pub failed: bool,
    pub in_progress: bool,
}

#[derive(Template)]
#[template(path = "platform/tenders/status.html")]
pub(crate) struct TenderStatusPage {
    pub cfg: Config,
    pub page_id: PageId,
    #[allow(dead_code)]
    pub user: User,
    pub job_id: Uuid,
    pub project_title: String,
    pub status_label: &'static str,
    pub error_label: Option<&'static str>,
    pub report_id: Option<Uuid>,
    pub failed: bool,
    pub in_progress: bool,
}

#[derive(Template)]
#[template(path = "platform/tenders/report.html")]
pub(crate) struct TenderReportPage {
    pub cfg: Config,
    pub page_id: PageId,
    #[allow(dead_code)]
    pub user: User,
    pub project_title: String,
    pub report_status: &'static str,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_versions: Vec<String>,
    pub facts: Vec<TenderFactView>,
    pub qualifications: Vec<TenderQualificationView>,
    pub risks: Vec<TenderRiskView>,
    pub materials: Vec<TenderMaterialView>,
    pub unknowns: Vec<TenderUnknownView>,
    pub sources: Vec<TenderCitationView>,
}

pub(crate) struct TenderCitationView {
    pub page_number: u16,
    pub quote: String,
}

pub(crate) struct TenderFactView {
    pub label: &'static str,
    pub value: String,
    pub confirmed: bool,
    pub citations: Vec<TenderCitationView>,
}

pub(crate) struct TenderQualificationView {
    pub kind: &'static str,
    pub requirement: String,
    pub original_text: String,
    pub citations: Vec<TenderCitationView>,
}

pub(crate) struct TenderRiskView {
    pub severity: &'static str,
    pub fact: String,
    pub judgment: String,
    pub impact: String,
    pub suggestion: String,
    pub citations: Vec<TenderCitationView>,
}

pub(crate) struct TenderMaterialView {
    pub name: String,
    pub description: String,
    pub source_requirement: String,
    pub citations: Vec<TenderCitationView>,
}

pub(crate) struct TenderUnknownView {
    pub field_name: String,
    pub reason: &'static str,
    pub detail: String,
    pub citations: Vec<TenderCitationView>,
}
