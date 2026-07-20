//! This module defines types to represent the server configuration.

use std::{collections::HashMap, fmt, path::PathBuf};

use anyhow::Result;
use deadpool_postgres::Config as DbConfig;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Yaml},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Main server configuration struct, containing all sub-configurations.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Config {
    /// Database configuration.
    pub db: DbConfig,
    /// Email configuration.
    pub email: EmailConfig,
    /// Logging configuration.
    pub log: LogConfig,
    /// HTTP server configuration.
    pub server: HttpServerConfig,
}

impl Config {
    /// Create a new Config instance, loading from file and environment.
    #[instrument(err)]
    pub(crate) fn new(config_file: Option<&PathBuf>) -> Result<Self> {
        let mut figment = Figment::new()
            .merge(Serialized::default("log.format", "json"))
            .merge(Serialized::default("server.addr", "127.0.0.1:9000"));

        if let Some(config_file) = config_file {
            figment = figment.merge(Yaml::file(config_file));
        }

        figment
            .merge(Env::prefixed("GITJOBS_").split("__"))
            .extract()
            .map_err(Into::into)
    }
}

/// Email configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct EmailConfig {
    /// Sender email address.
    pub from_address: String,
    /// Sender display name.
    pub from_name: String,
    /// SMTP server configuration.
    pub smtp: SmtpConfig,

    /// Optional whitelist of allowed recipient email addresses.
    pub rcpts_whitelist: Option<Vec<String>>,
}

/// SMTP server configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct SmtpConfig {
    /// SMTP server hostname.
    pub host: String,
    /// SMTP server port.
    pub port: u16,
    /// SMTP username.
    pub username: String,
    /// SMTP password.
    pub password: String,
}

/// Logging configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct LogConfig {
    /// Log output format.
    pub format: LogFormat,
}

/// Supported log output formats.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogFormat {
    /// JSON log format.
    Json,
    /// Human-readable log format.
    Pretty,
}

/// HTTP server configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct HttpServerConfig {
    /// Server bind address.
    pub addr: String,
    /// Base URL for the server.
    pub base_url: String,
    /// Disable referer header validation for image uploads.
    #[serde(default)]
    pub disable_referer_checks: bool,
    /// Login options configuration.
    pub login: LoginOptions,
    /// `OAuth2` providers configuration.
    pub oauth2: OAuth2Config,
    /// `Oidc` providers configuration.
    pub oidc: OidcConfig,
    /// Talent import data protection configuration.
    #[serde(default)]
    pub talent: TalentConfig,

    /// Optional analytics configuration.
    pub analytics: Option<AnalyticsConfig>,
    /// Optional basic authentication configuration.
    pub basic_auth: Option<BasicAuth>,
    /// Optional cookie configuration.
    pub cookie: Option<CookieConfig>,
    /// Optional Slack webhook URL to post new jobs published notifications.
    pub slack_webhook_url: Option<String>,
}

/// Analytics integration configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct AnalyticsConfig {
    /// Google Tag Manager ID.
    pub google_tag_id: Option<String>,
    /// Osano script URL for cookie consent.
    pub osano_script_url: Option<String>,
}

/// Basic authentication configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct BasicAuth {
    /// Whether basic authentication is enabled.
    pub enabled: bool,
    /// Username for basic authentication.
    pub username: String,
    /// Password for basic authentication.
    pub password: String,
}

/// Cookie settings configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct CookieConfig {
    /// Whether cookies should be secure (HTTPS only).
    pub secure: Option<bool>,
}

/// Login options enabled for the server.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LoginOptions {
    /// Enable email login.
    pub email: bool,
    /// Enable GitHub login.
    pub github: bool,
    /// Enable Linux Foundation login.
    pub linuxfoundation: bool,
}

/// Type alias for the `OAuth2` configuration section.
pub(crate) type OAuth2Config = HashMap<OAuth2Provider, OAuth2ProviderConfig>;

/// Supported `OAuth2` providers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OAuth2Provider {
    /// GitHub as an `OAuth2` provider.
    GitHub,
}

/// `OAuth2` provider configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct OAuth2ProviderConfig {
    /// Authorization endpoint URL.
    pub auth_url: String,
    /// `OAuth2` client ID.
    pub client_id: String,
    /// `OAuth2` client secret.
    pub client_secret: String,
    /// Redirect URI after authentication.
    pub redirect_uri: String,
    /// Scopes requested from the provider.
    pub scopes: Vec<String>,
    /// Token endpoint URL.
    pub token_url: String,
}

/// Type alias for the `Oidc` configuration section.
pub(crate) type OidcConfig = HashMap<OidcProvider, OidcProviderConfig>;

/// Supported `Oidc` providers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OidcProvider {
    /// Linux Foundation as an `Oidc` provider.
    LinuxFoundation,
}

/// `Oidc` provider configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct OidcProviderConfig {
    /// `Oidc` client ID.
    pub client_id: String,
    /// `Oidc` client secret.
    pub client_secret: String,
    /// `Oidc` issuer URL.
    pub issuer_url: String,
    /// Redirect URI after authentication.
    pub redirect_uri: String,
    /// Scopes requested from the provider.
    pub scopes: Vec<String>,
}

/// Talent import settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct TalentConfig {
    /// Enable public prototype data while the talent import flow is unavailable.
    #[serde(default)]
    pub prototype_data: bool,
    /// Encryption and contact deduplication secrets.
    #[serde(default)]
    pub encryption: TalentEncryptionConfig,
}

/// Encryption settings for protected talent source data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct TalentEncryptionConfig {
    /// Version used for new AES-256-GCM envelopes.
    pub active_version: Option<String>,
    /// AES-256 keys, keyed by their version.
    #[serde(default)]
    pub keys: HashMap<String, SecretValue>,
    /// Independent HMAC-SHA256 pepper for contact deduplication.
    pub contact_pepper: Option<SecretValue>,
}

/// A configuration secret that is never rendered in debug or serialized output.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SecretValue(String);

impl SecretValue {
    #[cfg(test)]
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl<'de> Deserialize<'de> for SecretValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self)
    }
}

impl Serialize for SecretValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::{SecretValue, TalentConfig, TalentEncryptionConfig};

    const TEST_SECRET: &str = "fictional-config-secret";

    #[test]
    fn talent_config_defaults_to_disabled_without_secrets() {
        let config = TalentConfig::default();

        assert!(!config.prototype_data);
        assert!(config.encryption.active_version.is_none());
        assert!(config.encryption.keys.is_empty());
        assert!(config.encryption.contact_pepper.is_none());
    }

    #[test]
    fn secret_values_are_redacted_in_debug_and_serialized_output() {
        let secret: SecretValue = serde_json::from_str(&format!("\"{TEST_SECRET}\"")).unwrap();
        let config = TalentEncryptionConfig {
            active_version: Some("test-v1".to_owned()),
            keys: [("test-v1".to_owned(), secret.clone())].into_iter().collect(),
            contact_pepper: Some(secret),
        };
        let debug = format!("{config:?}");
        let serialized = serde_json::to_string(&config).unwrap();

        assert!(debug.contains("[REDACTED]"));
        assert!(serialized.contains("[REDACTED]"));
        assert!(!debug.contains(TEST_SECRET));
        assert!(!serialized.contains(TEST_SECRET));
    }
}
