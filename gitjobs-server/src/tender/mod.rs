//! Tender document analysis boundary.

pub(crate) mod client;
pub(crate) mod model;
pub(crate) mod processor;
pub(crate) mod storage;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::{config::TenderConfig, db::DynDB};

use self::{
    client::{HttpTenderAnalysisClient, TenderAnalysisClient},
    processor::TenderProcessor,
    storage::{FileSystemTenderDocumentStore, TenderDocumentStore},
};

/// One shared service graph for the worker and authenticated routes.
pub(crate) struct TenderServices {
    pub storage: Option<Arc<dyn TenderDocumentStore>>,
    pub processor: Option<Arc<TenderProcessor>>,
    pub analysis_config_hash: Option<String>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
}

impl TenderServices {
    pub(crate) fn disabled() -> Arc<Self> {
        Arc::new(Self {
            storage: None,
            processor: None,
            analysis_config_hash: None,
            model_provider: None,
            model_name: None,
        })
    }

    pub(crate) async fn from_config(db: DynDB, config: &TenderConfig) -> Result<Arc<Self>> {
        let Some(settings) = config.settings.as_ref().filter(|_| config.enabled) else {
            return Ok(Self::disabled());
        };
        let storage: Arc<dyn TenderDocumentStore> = Arc::new(
            FileSystemTenderDocumentStore::new(&settings.document_dir, settings.max_upload_bytes).await?,
        );
        let client: Arc<dyn TenderAnalysisClient> = Arc::new(HttpTenderAnalysisClient::new(
            settings.analysis_base_url.clone(),
            settings.internal_service_token.clone(),
            Duration::from_secs(settings.request_timeout_seconds),
        )?);
        let processor = Arc::new(TenderProcessor::new(
            db,
            storage.clone(),
            client,
            Duration::from_secs(settings.request_timeout_seconds),
            settings.model_provider.clone(),
            settings.model_name.clone(),
        ));
        let fingerprint = format!(
            "tender-analysis.v1\0{}\0{}\0tender-project-info-v1\0tender-qualifications-v1\0tender-risks-v1\0tender-materials-v1\0tender-review-v1",
            settings.model_provider, settings.model_name
        );
        let analysis_config_hash = hex_bytes(&Sha256::digest(fingerprint.as_bytes()));
        Ok(Arc::new(Self {
            storage: Some(storage),
            processor: Some(processor),
            analysis_config_hash: Some(analysis_config_hash),
            model_provider: Some(settings.model_provider.clone()),
            model_name: Some(settings.model_name.clone()),
        }))
    }
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::hex_bytes;

    #[test]
    fn hex_bytes_encodes_digest_without_sha2_formatting_traits() {
        assert_eq!(hex_bytes(&[0x00, 0x0f, 0x10, 0xab, 0xff]), "000f10abff");
    }
}
