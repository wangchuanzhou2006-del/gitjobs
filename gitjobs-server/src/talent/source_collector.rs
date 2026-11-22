use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
    time::Duration,
};

use feed_rs::parser;
use reqwest::{
    Client, StatusCode,
    header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED},
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    db::{
        DynDB,
        talent::{CompleteTalentSourceRun, TalentSourceSubscription},
    },
    talent::{ImportService, NewImport, SourceType, validate_source_subscription},
};

const MAX_FEED_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedEntry {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub(crate) enum SourceCollectError {
    #[error("unsafe source")]
    UnsafeSource,
    #[error("source request failed")]
    Fetch,
    #[error("source response is too large")]
    TooLarge,
    #[error("source feed is invalid")]
    InvalidFeed,
    #[error("source persistence failed")]
    Persistence,
}

impl SourceCollectError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::UnsafeSource => "unsafe_source",
            Self::Fetch => "fetch_failed",
            Self::TooLarge => "response_too_large",
            Self::InvalidFeed => "invalid_feed",
            Self::Persistence => "persistence_failed",
        }
    }
}

pub(crate) struct TalentSourceCollector {
    db: DynDB,
    import: Arc<ImportService>,
    client: Client,
    allowed_hosts: Vec<String>,
}

impl TalentSourceCollector {
    pub(crate) fn new(
        db: DynDB,
        import: Arc<ImportService>,
        allowed_hosts: Vec<String>,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            db,
            import,
            client,
            allowed_hosts,
        })
    }

    pub(crate) async fn collect_one(&self) -> Result<bool, SourceCollectError> {
        let Some(claim) = self
            .db
            .claim_talent_source_subscription()
            .await
            .map_err(|_| SourceCollectError::Persistence)?
        else {
            return Ok(false);
        };
        let result = self.collect(&claim.subscription).await;
        let completion = match &result {
            Ok(outcome) => CompleteTalentSourceRun {
                run_id: claim.run_id,
                discovered_count: outcome.discovered,
                accepted_count: outcome.accepted,
                duplicate_count: outcome.duplicates,
                failed_count: outcome.failed,
                error_code: None,
                incremental_cursor: outcome.cursor.clone(),
                etag: outcome.etag.clone(),
                last_modified: outcome.last_modified.clone(),
            },
            Err(error) => CompleteTalentSourceRun {
                run_id: claim.run_id,
                discovered_count: 0,
                accepted_count: 0,
                duplicate_count: 0,
                failed_count: 1,
                error_code: Some(error.code().to_owned()),
                incremental_cursor: None,
                etag: None,
                last_modified: None,
            },
        };
        self.db
            .complete_talent_source_run(completion)
            .await
            .map_err(|_| SourceCollectError::Persistence)?;
        result.map(|_| true)
    }

    async fn collect(&self, source: &TalentSourceSubscription) -> Result<CollectOutcome, SourceCollectError> {
        let validated = validate_source_subscription(
            &source.public_url,
            &source.keywords,
            source.interval_minutes,
            &self.allowed_hosts,
        )
        .map_err(|_| SourceCollectError::UnsafeSource)?;
        verify_public_dns(&validated.public_url).await?;
        let mut request = self.client.get(validated.public_url);
        if let Some(etag) = &source.etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(modified) = &source.last_modified {
            request = request.header(IF_MODIFIED_SINCE, modified);
        }
        let mut response = request.send().await.map_err(|_| SourceCollectError::Fetch)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(CollectOutcome::default());
        }
        if !response.status().is_success() {
            return Err(SourceCollectError::Fetch);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_FEED_BYTES as u64)
        {
            return Err(SourceCollectError::TooLarge);
        }
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let last_modified = response
            .headers()
            .get(LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| SourceCollectError::Fetch)? {
            append_limited(&mut bytes, &chunk)?;
        }
        let entries = parse_feed(&bytes)?;
        let actor = source.created_by.ok_or(SourceCollectError::Persistence)?;
        let mut outcome = CollectOutcome {
            discovered: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            cursor: entries.first().map(|entry| entry.id.clone()),
            etag,
            last_modified,
            ..CollectOutcome::default()
        };
        for entry in entries {
            if source.incremental_cursor.as_deref() == Some(entry.id.as_str()) {
                outcome.duplicates += 1;
                break;
            }
            if !matches_keywords(&entry.text, &validated.keywords) {
                continue;
            }
            let fingerprint = content_fingerprint(&entry.text);
            if !self
                .db
                .reserve_talent_source_item(source.subscription_id, &entry.id, &fingerprint)
                .await
                .map_err(|_| SourceCollectError::Persistence)?
            {
                outcome.duplicates += 1;
                continue;
            }
            if self
                .import
                .create_batch(
                    actor,
                    NewImport {
                        source_type: SourceType::Web,
                        source_name: source.source_name.clone(),
                        raw_text: entry.text,
                    },
                )
                .await
                .is_ok()
            {
                outcome.accepted += 1;
            } else {
                outcome.failed += 1;
                self.db
                    .release_talent_source_item(source.subscription_id, &entry.id, &fingerprint)
                    .await
                    .map_err(|_| SourceCollectError::Persistence)?;
            }
        }
        Ok(outcome)
    }
}

fn append_limited(target: &mut Vec<u8>, chunk: &[u8]) -> Result<(), SourceCollectError> {
    if target.len().saturating_add(chunk.len()) > MAX_FEED_BYTES {
        return Err(SourceCollectError::TooLarge);
    }
    target.extend_from_slice(chunk);
    Ok(())
}

#[derive(Debug, Default)]
struct CollectOutcome {
    discovered: u32,
    accepted: u32,
    duplicates: u32,
    failed: u32,
    cursor: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
}

pub(crate) fn parse_feed(bytes: &[u8]) -> Result<Vec<FeedEntry>, SourceCollectError> {
    let feed = parser::parse(bytes).map_err(|_| SourceCollectError::InvalidFeed)?;
    Ok(feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title.map(|value| value.content).unwrap_or_default();
            let body = entry
                .summary
                .map(|value| value.content)
                .or_else(|| entry.content.and_then(|value| value.body))
                .unwrap_or_default();
            let text = format!("{title}\n{body}").trim().to_owned();
            (!entry.id.trim().is_empty() && !text.is_empty()).then_some(FeedEntry { id: entry.id, text })
        })
        .collect())
}

fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    let normalized = text.to_lowercase();
    keywords.iter().any(|keyword| normalized.contains(keyword))
}

#[allow(clippy::format_collect)]
fn content_fingerprint(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    Sha256::digest(normalized.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn verify_public_dns(url: &reqwest::Url) -> Result<(), SourceCollectError> {
    let host = url.host_str().ok_or(SourceCollectError::UnsafeSource)?;
    let addresses = tokio::net::lookup_host((host, 443))
        .await
        .map_err(|_| SourceCollectError::Fetch)?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(SourceCollectError::UnsafeSource);
    }
    Ok(())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || octets[0] == 0
        || octets[0] >= 224
        || octets[0] == 100 && (64..=127).contains(&octets[1])
        || octets[0] == 192 && octets[1] == 0 && octets[2] == 0
        || octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80)
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{
        MAX_FEED_BYTES, SourceCollectError, append_limited, content_fingerprint, is_public_ip,
        matches_keywords, parse_feed,
    };

    #[test]
    fn parses_rss_and_filters_normalized_keywords() {
        let feed = br#"<?xml version="1.0"?><rss version="2.0"><channel><title>Jobs</title><item><guid>item-1</guid><title>Kitchen Cook</title><description>Beijing role</description></item></channel></rss>"#;
        let entries = parse_feed(feed).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "item-1");
        assert!(matches_keywords(&entries[0].text, &["cook".to_owned()]));
    }

    #[test]
    fn parses_atom_entries() {
        let feed = br#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Jobs</title><id>feed</id><updated>2026-01-01T00:00:00Z</updated><entry><id>atom-1</id><title>Chef</title><updated>2026-01-01T00:00:00Z</updated><summary>Shanghai kitchen</summary></entry></feed>"#;
        assert_eq!(parse_feed(feed).unwrap()[0].id, "atom-1");
    }

    #[test]
    fn rejects_malformed_feed_content() {
        assert!(matches!(
            parse_feed(b"not a feed"),
            Err(SourceCollectError::InvalidFeed)
        ));
    }

    #[test]
    fn rejects_private_and_reserved_network_destinations() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.1.1",
            "192.168.1.1",
            "100.64.0.1",
            "224.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
        ] {
            assert!(!is_public_ip(address.parse::<IpAddr>().unwrap()), "{address}");
        }
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn enforces_the_response_limit_without_a_content_length_header() {
        let mut body = vec![0; MAX_FEED_BYTES - 2];
        append_limited(&mut body, &[1, 2]).unwrap();
        assert!(matches!(
            append_limited(&mut body, &[3]),
            Err(SourceCollectError::TooLarge)
        ));
        assert_eq!(body.len(), MAX_FEED_BYTES);
    }

    #[test]
    fn content_fingerprints_ignore_case_and_spacing() {
        assert_eq!(
            content_fingerprint(" Chef  Job\nBeijing "),
            content_fingerprint("chef job beijing")
        );
        assert_ne!(content_fingerprint("chef job"), content_fingerprint("cook job"));
    }
}
