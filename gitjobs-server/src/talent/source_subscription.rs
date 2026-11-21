use std::{collections::BTreeSet, net::IpAddr};

use reqwest::Url;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedSourceSubscription {
    pub public_url: Url,
    pub keywords: Vec<String>,
    pub interval_minutes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum SourceSubscriptionError {
    #[error("source URL is not an approved public HTTPS address")]
    UnsafeUrl,
    #[error("source host is not allowed")]
    HostNotAllowed,
    #[error("source keywords are invalid")]
    InvalidKeywords,
    #[error("source update interval is invalid")]
    InvalidInterval,
}

pub(crate) fn validate_source_subscription(
    public_url: &str,
    keywords: &[String],
    interval_minutes: u32,
    allowed_hosts: &[String],
) -> Result<ValidatedSourceSubscription, SourceSubscriptionError> {
    let url = Url::parse(public_url).map_err(|_| SourceSubscriptionError::UnsafeUrl)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some()
    {
        return Err(SourceSubscriptionError::UnsafeUrl);
    }
    let host = url
        .host_str()
        .ok_or(SourceSubscriptionError::UnsafeUrl)?
        .to_ascii_lowercase();
    let ip_host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(&host);
    if host == "localhost" || ip_host.parse::<IpAddr>().is_ok() {
        return Err(SourceSubscriptionError::UnsafeUrl);
    }
    if !allowed_hosts
        .iter()
        .any(|allowed| host == allowed.trim().to_ascii_lowercase())
    {
        return Err(SourceSubscriptionError::HostNotAllowed);
    }
    if !(360..=10_080).contains(&interval_minutes) {
        return Err(SourceSubscriptionError::InvalidInterval);
    }
    let normalized = keywords
        .iter()
        .map(|keyword| keyword.trim().to_lowercase())
        .filter(|keyword| !keyword.is_empty() && keyword.chars().count() <= 50)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized.is_empty() || normalized.len() > 20 {
        return Err(SourceSubscriptionError::InvalidKeywords);
    }
    Ok(ValidatedSourceSubscription {
        public_url: url,
        keywords: normalized,
        interval_minutes,
    })
}

#[cfg(test)]
mod tests {
    use super::{SourceSubscriptionError, validate_source_subscription};

    fn keywords() -> Vec<String> {
        vec![" 厨师 ".to_owned(), "厨师".to_owned(), "切配".to_owned()]
    }

    #[test]
    fn accepts_only_allowlisted_public_https_sources() {
        let source = validate_source_subscription(
            "https://jobs.example.test/feed.xml",
            &keywords(),
            720,
            &["jobs.example.test".to_owned()],
        )
        .unwrap();
        assert_eq!(source.keywords, ["切配", "厨师"]);
        assert_eq!(source.interval_minutes, 720);
    }

    #[test]
    fn rejects_private_or_credential_bearing_addresses_before_fetching() {
        for url in [
            "http://jobs.example.test/feed",
            "https://user:pass@jobs.example.test/feed",
            "https://jobs.example.test/feed#private",
            "https://localhost/feed",
            "https://127.0.0.1/feed",
            "https://[::1]/feed",
        ] {
            assert_eq!(
                validate_source_subscription(url, &keywords(), 720, &["jobs.example.test".to_owned()]),
                Err(SourceSubscriptionError::UnsafeUrl),
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_hosts_outside_the_server_allowlist() {
        assert_eq!(
            validate_source_subscription(
                "https://other.example.test/feed",
                &keywords(),
                720,
                &["jobs.example.test".to_owned()]
            ),
            Err(SourceSubscriptionError::HostNotAllowed)
        );
    }
}
