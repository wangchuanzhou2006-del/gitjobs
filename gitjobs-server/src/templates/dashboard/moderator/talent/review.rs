//! Presentation models for the talent review queue and detail page.

use askama::Template;
use axum_messages::{Level, Message};
use uuid::Uuid;

use crate::templates::{Config, PageId, auth::User, dashboard::moderator::Menu, filters};

#[derive(Debug, Clone, Template)]
#[template(path = "dashboard/moderator/talent/review.html")]
pub(crate) struct QueuePage {
    pub cfg: Config,
    pub page_id: PageId,
    pub messages: Vec<Message>,
    pub user: User,
    pub auth_provider: Option<String>,
    pub moderator_menu: Menu,
    pub items: Vec<QueueItem>,
    pub selected_status: String,
    pub selected_kind: String,
    pub selected_risk: String,
    pub selected_source: String,
    pub selected_time: String,
    pub selected_batch: String,
}

#[derive(Debug, Clone)]
pub(crate) struct QueueItem {
    pub item_id: Uuid,
    #[allow(dead_code)]
    pub source_id: Uuid,
    pub kind_label: String,
    pub title: String,
    pub region_label: String,
    pub role_label: String,
    pub salary_label: String,
    pub completeness_label: String,
    pub confidence_label: String,
    pub risk_label: String,
    pub risk_high: bool,
    pub contact_label: String,
    pub duplicate_label: String,
    pub source_name: String,
    pub updated_label: String,
    pub status_label: String,
}

#[derive(Debug, Clone, Template)]
#[template(path = "dashboard/moderator/talent/review_item.html")]
pub(crate) struct DetailPage {
    pub cfg: Config,
    pub page_id: PageId,
    pub messages: Vec<Message>,
    pub user: User,
    pub auth_provider: Option<String>,
    pub moderator_menu: Menu,
    pub item: DetailItem,
}

#[derive(Clone)]
pub(crate) struct DetailItem {
    pub item_id: Uuid,
    pub source_id: Uuid,
    pub runtime_updated_at: String,
    pub source_updated_at: String,
    pub published_record_id: Option<Uuid>,
    pub lock_version: u32,
    pub raw_fragment: String,
    pub kind_label: String,
    pub title: String,
    pub city: String,
    pub district: String,
    pub region: String,
    pub roles: String,
    pub skills: String,
    pub salary: String,
    pub confidence_label: String,
    pub risk_label: String,
    #[allow(dead_code)]
    pub source_name: String,
    pub status_label: String,
    pub contact_label: String,
    pub duplicate_label: String,
    pub risk_reasons: Vec<RiskReasonItem>,
}

#[derive(Debug, Clone)]
pub(crate) struct RiskReasonItem {
    pub explanation: String,
    pub evidence: Option<String>,
    pub hard_block: bool,
}

impl std::fmt::Debug for DetailItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DetailItem")
            .field("item_id", &self.item_id)
            .field("lock_version", &self.lock_version)
            .field("raw_fragment", &"[REDACTED]")
            .field("kind_label", &self.kind_label)
            .field("title", &self.title)
            .field("status_label", &self.status_label)
            .finish_non_exhaustive()
    }
}
