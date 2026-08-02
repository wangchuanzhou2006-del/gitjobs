use askama::Template;
use axum_messages::{Level, Message};

use crate::templates::{Config, PageId, auth::User, dashboard::moderator::Menu, filters};

#[derive(Debug, Clone, Template)]
#[template(path = "dashboard/moderator/talent/radar.html")]
pub(crate) struct RadarPage {
    pub cfg: Config,
    pub page_id: PageId,
    pub messages: Vec<Message>,
    pub user: User,
    pub auth_provider: Option<String>,
    pub moderator_menu: Menu,
    pub enabled: bool,
    pub watched_count: usize,
    pub attention_count: usize,
    pub sources: Vec<RadarSource>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RadarSource {
    pub subscription_id: uuid::Uuid,
    pub name: String,
    pub topics: String,
    pub frequency: String,
    pub updated: String,
    pub status: String,
    pub needs_attention: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Template)]
#[template(path = "dashboard/moderator/talent/radar_edit.html")]
pub(crate) struct RadarEditPage {
    pub cfg: Config,
    pub page_id: PageId,
    #[allow(dead_code)]
    pub messages: Vec<Message>,
    pub user: User,
    pub auth_provider: Option<String>,
    pub moderator_menu: Menu,
    pub subscription_id: uuid::Uuid,
    pub source_name: String,
    pub public_url: String,
    pub keywords: String,
    pub frequency: String,
    pub enabled: bool,
}
