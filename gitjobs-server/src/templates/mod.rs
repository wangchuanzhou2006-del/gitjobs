//! This module defines templates for rendering various job board components.

use serde::{Deserialize, Serialize};

use crate::config::{AnalyticsConfig, HttpServerConfig};

pub(crate) mod auth;
pub(crate) mod dashboard;
pub(crate) mod filters;
pub(crate) mod helpers;
pub(crate) mod jobboard;
pub(crate) mod misc;
pub(crate) mod notifications;
pub(crate) mod pagination;
pub(crate) mod platform;

/// Subset of the server configuration used in templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub analytics: Option<AnalyticsConfig>,
    pub tender_enabled: bool,
}

impl From<HttpServerConfig> for Config {
    fn from(cfg: HttpServerConfig) -> Self {
        Self {
            analytics: cfg.analytics,
            tender_enabled: cfg.tender.enabled,
        }
    }
}

/// Enum representing unique identifiers for each page in the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PageId {
    About,
    EmployerDashboard,
    JobBoard,
    JobSeekerDashboard,
    LogIn,
    ModeratorDashboard,
    NotFound,
    Opportunities,
    Talent,
    Tenders,
    SignUp,
    Stats,
}
