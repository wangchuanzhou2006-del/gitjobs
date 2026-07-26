//! This module defines the templates for the moderator dashboard.

pub(crate) mod home;
pub(crate) mod jobs;
pub(crate) mod talent;

/// Shared active-state flags for the moderator navigation.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Menu {
    pub live_jobs: bool,
    pub pending_jobs: bool,
    pub talent_import: bool,
    pub talent_review: bool,
}
