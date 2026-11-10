//! This module defines types to represent the server configuration.

use std::{collections::HashMap, fmt, net::IpAddr, path::PathBuf};

use anyhow::Result;
use deadpool_postgres::Config as DbConfig;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Yaml},
};
use serde::{Deserialize, Serialize, de::Error as _};
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct TalentConfig {
    /// Enable the OpenAI-compatible talent extraction provider.
    #[serde(default)]
    pub enabled: bool,
    /// Enable public prototype data while the talent import flow is unavailable.
    #[serde(default)]
    pub prototype_data: bool,
    /// Enable scheduled updates from approved public sources.
    #[serde(default)]
    pub source_updates_enabled: bool,
    /// Exact public hostnames that scheduled updates may access.
    #[serde(default)]
    pub allowed_public_hosts: Vec<String>,
    /// Encryption and contact deduplication secrets.
    #[serde(default)]
    pub encryption: TalentEncryptionConfig,
    /// Optional OpenAI-compatible extraction provider settings.
    #[serde(default)]
    pub ai: Option<TalentAiConfig>,
}

impl TalentConfig {
    /// Return provider settings only when talent AI is explicitly enabled.
    pub(crate) fn ai_config(&self) -> Option<&TalentAiConfig> {
        self.enabled.then_some(self.ai.as_ref()).flatten()
    }
}

#[derive(Deserialize)]
struct TalentConfigRepr {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    prototype_data: bool,
    #[serde(default)]
    source_updates_enabled: bool,
    #[serde(default)]
    allowed_public_hosts: Vec<String>,
    #[serde(default)]
    encryption: TalentEncryptionConfig,
    #[serde(default)]
    ai: Option<TalentAiConfigRepr>,
}

#[derive(Deserialize)]
struct TalentAiConfigRepr {
    base_url: Option<String>,
    #[serde(default)]
    allow_local_http: bool,
    model: Option<String>,
    api_key: Option<SecretValue>,
}

impl<'de> Deserialize<'de> for TalentConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let repr = TalentConfigRepr::deserialize(deserializer)?;
        let ai = if repr.enabled {
            let ai = repr.ai.ok_or_else(|| D::Error::missing_field("ai"))?;
            let raw_base_url = ai.base_url.ok_or_else(|| D::Error::missing_field("ai.base_url"))?;
            Some(TalentAiConfig {
                base_url: validate_talent_ai_base_url(&raw_base_url, ai.allow_local_http)
                    .map_err(D::Error::custom)?,
                model: ai.model.ok_or_else(|| D::Error::missing_field("ai.model"))?,
                api_key: ai.api_key.ok_or_else(|| D::Error::missing_field("ai.api_key"))?,
            })
        } else {
            None
        };

        Ok(Self {
            enabled: repr.enabled,
            prototype_data: repr.prototype_data,
            source_updates_enabled: repr.source_updates_enabled,
            allowed_public_hosts: repr.allowed_public_hosts,
            encryption: repr.encryption,
            ai,
        })
    }
}

fn validate_talent_ai_base_url(value: &str, allow_local_http: bool) -> Result<reqwest::Url, &'static str> {
    let mut url = reqwest::Url::parse(value).map_err(|_| "talent AI base URL is invalid")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("talent AI base URL cannot contain credentials, query, or fragment");
    }
    if !matches!(url.path(), "/" | "/v1" | "/v1/") {
        return Err("talent AI base URL path must be root or /v1");
    }

    match url.scheme() {
        "https" => {}
        "http" if allow_local_http && talent_ai_url_is_loopback(&url) => {}
        _ => return Err("talent AI base URL must use HTTPS"),
    }

    if url.path() == "/v1" {
        url.set_path("/v1/");
    }
    Ok(url)
}

fn talent_ai_url_is_loopback(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|address| address.is_loopback())
}

/// OpenAI-compatible talent extraction provider settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TalentAiConfig {
    pub base_url: reqwest::Url,
    pub model: String,
    pub api_key: SecretValue,
}

/// Encryption settings for protected talent source data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct TalentEncryptionConfig {
    /// Version used for new AES-256-GCM envelopes.
    #[serde(default, deserialize_with = "deserialize_optional_string_or_u64")]
    pub active_version: Option<String>,
    /// AES-256 keys, keyed by their version.
    #[serde(default)]
    pub keys: HashMap<String, SecretValue>,
    /// Independent HMAC-SHA256 pepper for contact deduplication.
    pub contact_pepper: Option<SecretValue>,
}

fn deserialize_optional_string_or_u64<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrU64 {
        String(String),
        U64(u64),
    }

    Option::<StringOrU64>::deserialize(deserializer).map(|value| {
        value.map(|value| match value {
            StringOrU64::String(value) => value,
            StringOrU64::U64(value) => value.to_string(),
        })
    })
}

/// A configuration secret that is never rendered in debug or serialized output.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SecretValue(String);

impl SecretValue {
    #[cfg(test)]
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[allow(dead_code)]
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
    use std::path::PathBuf;

    use super::{Config, SecretValue, TalentConfig, TalentEncryptionConfig};

    const TEST_SECRET: &str = "fictional-config-secret";

    #[test]
    fn talent_config_defaults_to_disabled_without_secrets() {
        let config = TalentConfig::default();

        assert!(!config.prototype_data);
        assert!(!config.source_updates_enabled);
        assert!(config.allowed_public_hosts.is_empty());
        assert!(config.encryption.active_version.is_none());
        assert!(config.ai.is_none());
        assert!(config.encryption.keys.is_empty());
        assert!(config.encryption.contact_pepper.is_none());
    }

    #[test]
    fn numeric_talent_key_version_deserializes_to_its_canonical_string() {
        let encryption: TalentEncryptionConfig = serde_json::from_value(serde_json::json!({
            "active_version": 1,
        }))
        .expect("numeric environment-style key version should deserialize");

        assert_eq!(encryption.active_version.as_deref(), Some("1"));
    }

    #[test]
    fn disabled_local_example_loads_without_an_ai_api_key() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/examples/server.local.yml");

        let config = Config::new(Some(&path)).expect("disabled example should load without secrets");

        assert!(!config.server.talent.enabled);
        assert!(config.server.talent.ai_config().is_none());
    }

    #[test]
    fn enabled_talent_ai_requires_every_provider_setting() {
        for incomplete in [
            serde_json::json!({"enabled": true}),
            serde_json::json!({
                "enabled": true,
                "ai": {"model": "test-model", "api_key": TEST_SECRET},
            }),
            serde_json::json!({
                "enabled": true,
                "ai": {"base_url": "https://api.example.invalid/v1", "api_key": TEST_SECRET},
            }),
            serde_json::json!({
                "enabled": true,
                "ai": {
                    "base_url": "https://api.example.invalid/v1",
                    "model": "test-model",
                },
            }),
        ] {
            assert!(serde_json::from_value::<TalentConfig>(incomplete).is_err());
        }
    }

    #[test]
    fn enabled_talent_ai_exposes_only_a_complete_provider_config() {
        let config: TalentConfig = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "ai": {
                "base_url": "https://api.example.invalid/v1",
                "model": "test-model",
                "api_key": TEST_SECRET,
            },
        }))
        .unwrap();

        let ai = config.ai_config().unwrap();
        assert_eq!(ai.base_url.as_str(), "https://api.example.invalid/v1/");
        assert_eq!(ai.model, "test-model");
        assert_eq!(ai.api_key.expose_secret(), TEST_SECRET);
    }

    #[test]
    fn enabled_talent_ai_rejects_unsafe_or_ambiguous_provider_urls() {
        for base_url in [
            "not a URL",
            "http://api.example.invalid/v1",
            "https://user:password@api.example.invalid/v1",
            "https://api.example.invalid/v1?tenant=secret",
            "https://api.example.invalid/v1#fragment",
            "https://api.example.invalid/v1/chat/completions",
        ] {
            let value = serde_json::json!({
                "enabled": true,
                "ai": {
                    "base_url": base_url,
                    "model": "test-model",
                    "api_key": TEST_SECRET,
                },
            });

            assert!(
                serde_json::from_value::<TalentConfig>(value).is_err(),
                "provider URL should fail closed: {base_url}"
            );
        }
    }

    #[test]
    fn local_http_provider_requires_an_explicit_loopback_only_opt_in() {
        let without_opt_in = serde_json::json!({
            "enabled": true,
            "ai": {
                "base_url": "http://127.0.0.1:8080/v1",
                "model": "test-model",
                "api_key": TEST_SECRET,
            },
        });
        let external_with_opt_in = serde_json::json!({
            "enabled": true,
            "ai": {
                "base_url": "http://api.example.invalid/v1",
                "allow_local_http": true,
                "model": "test-model",
                "api_key": TEST_SECRET,
            },
        });
        let loopback_with_opt_in = serde_json::json!({
            "enabled": true,
            "ai": {
                "base_url": "http://127.0.0.1:8080/v1",
                "allow_local_http": true,
                "model": "test-model",
                "api_key": TEST_SECRET,
            },
        });

        assert!(serde_json::from_value::<TalentConfig>(without_opt_in).is_err());
        assert!(serde_json::from_value::<TalentConfig>(external_with_opt_in).is_err());
        let config = serde_json::from_value::<TalentConfig>(loopback_with_opt_in).unwrap();
        assert_eq!(
            config.ai_config().unwrap().base_url.as_str(),
            "http://127.0.0.1:8080/v1/"
        );
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
