//! Talent import page templates and presentation models.

use askama::Template;
use axum_messages::{Level, Message};
use uuid::Uuid;

use crate::templates::{Config, PageId, auth::User, dashboard::moderator::Menu, filters};

#[derive(Debug, Clone, Template)]
#[template(path = "dashboard/moderator/talent/import.html")]
pub(crate) struct Page {
    pub cfg: Config,
    pub page_id: PageId,
    pub messages: Vec<Message>,
    pub user: User,
    pub auth_provider: Option<String>,
    pub moderator_menu: Menu,
    pub batches: Vec<Batch>,
    pub errors: Vec<String>,
    pub source_type: String,
    pub source_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Batch {
    pub batch_id: Uuid,
    pub source_name: String,
    pub source_label: String,
    pub status_label: String,
    pub progress_label: String,
    pub created_label: String,
    pub failed: bool,
}
