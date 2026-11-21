//! Bounded, authenticated calls to the stateless internal analysis service.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url, header};
use thiserror::Error;
use uuid::Uuid;

use crate::config::SecretValue;

use super::model::{AnalyzeRequest, ParseResponse, ParseStatus, ParsedDocument, TenderAnalysisV1};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientError {
    #[error("analysis_unavailable")]
    Unavailable,
    #[error("analysis_unavailable")]
    Unauthorized,
    #[error("invalid_pdf")]
    InvalidPdf,
    #[error("encrypted_pdf")]
    EncryptedPdf,
    #[error("too_many_pages")]
    TooManyPages,
    #[error("extracted_text_too_large")]
    ExtractedTextTooLarge,
    #[error("ocr_required")]
    OcrRequired,
    #[error("invalid_analysis_contract")]
    InvalidContract,
}

impl ClientError {
    pub(crate) fn is_retryable(self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

#[async_trait]
pub(crate) trait TenderAnalysisClient: Send + Sync {
    async fn parse(&self, pdf: &[u8]) -> Result<ParsedDocument, ClientError>;
    async fn analyze(&self, job_id: Uuid, document: &ParsedDocument)
    -> Result<TenderAnalysisV1, ClientError>;
}

pub(crate) struct HttpTenderAnalysisClient {
    http: Client,
    base_url: Url,
    token: SecretValue,
}

impl HttpTenderAnalysisClient {
    pub(crate) fn new(base_url: Url, token: SecretValue, timeout: Duration) -> Result<Self, ClientError> {
        let http = Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ClientError::Unavailable)?;
        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    async fn bounded_response(mut response: reqwest::Response) -> Result<Vec<u8>, ClientError> {
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(ClientError::InvalidContract);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ClientError::Unavailable)? {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err(ClientError::InvalidContract);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    fn endpoint(&self, path: &str) -> Result<Url, ClientError> {
        self.base_url.join(path).map_err(|_| ClientError::Unavailable)
    }
}

#[async_trait]
impl TenderAnalysisClient for HttpTenderAnalysisClient {
    async fn parse(&self, pdf: &[u8]) -> Result<ParsedDocument, ClientError> {
        if pdf.len() > 50 * 1024 * 1024 {
            return Err(ClientError::InvalidPdf);
        }
        let boundary = Uuid::new_v4();
        let prefix = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"document.pdf\"\r\nContent-Type: application/pdf\r\n\r\n"
        );
        let suffix = format!("\r\n--{boundary}--\r\n");
        let mut body = Vec::with_capacity(prefix.len() + pdf.len() + suffix.len());
        body.extend_from_slice(prefix.as_bytes());
        body.extend_from_slice(pdf);
        body.extend_from_slice(suffix.as_bytes());
        let response = self
            .http
            .post(self.endpoint("/internal/v1/parse")?)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .bearer_auth(self.token.expose_secret())
            .body(body)
            .send()
            .await
            .map_err(|_| ClientError::Unavailable)?;
        let status = response.status();
        let data = Self::bounded_response(response).await?;
        if status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY {
            let parsed: ParseResponse =
                serde_json::from_slice(&data).map_err(|_| ClientError::InvalidContract)?;
            parsed.validate().map_err(|_| ClientError::InvalidContract)?;
            return match (status, parsed.status, parsed.parsed_document, parsed.error_code) {
                (StatusCode::OK, ParseStatus::Completed, Some(document), None) => Ok(document),
                (StatusCode::UNPROCESSABLE_ENTITY, ParseStatus::Failed, None, Some(code)) => {
                    Err(match code {
                        super::model::ParseErrorCode::InvalidPdf => ClientError::InvalidPdf,
                        super::model::ParseErrorCode::EncryptedPdf => ClientError::EncryptedPdf,
                        super::model::ParseErrorCode::TooManyPages => ClientError::TooManyPages,
                        super::model::ParseErrorCode::ExtractedTextTooLarge => {
                            ClientError::ExtractedTextTooLarge
                        }
                        super::model::ParseErrorCode::OcrRequired => ClientError::OcrRequired,
                    })
                }
                _ => Err(ClientError::InvalidContract),
            };
        }
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(ClientError::Unauthorized);
        }
        if status.is_client_error() {
            return Err(ClientError::InvalidContract);
        }
        Err(ClientError::Unavailable)
    }

    async fn analyze(
        &self,
        job_id: Uuid,
        document: &ParsedDocument,
    ) -> Result<TenderAnalysisV1, ClientError> {
        document.validate().map_err(|_| ClientError::InvalidContract)?;
        let request = AnalyzeRequest {
            job_id: job_id.to_string(),
            document,
        };
        let body = serde_json::to_vec(&request).map_err(|_| ClientError::InvalidContract)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ClientError::InvalidContract);
        }
        let response = self
            .http
            .post(self.endpoint("/internal/v1/analyze")?)
            .bearer_auth(self.token.expose_secret())
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| ClientError::Unavailable)?;
        let status = response.status();
        let data = Self::bounded_response(response).await?;
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(ClientError::Unauthorized);
        }
        if status.is_client_error() {
            return Err(ClientError::InvalidContract);
        }
        if status != StatusCode::OK {
            return Err(ClientError::Unavailable);
        }
        let report: TenderAnalysisV1 =
            serde_json::from_slice(&data).map_err(|_| ClientError::InvalidContract)?;
        report
            .validate_against(document)
            .map_err(|_| ClientError::InvalidContract)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        Router,
        body::{Body, Bytes},
        extract::State,
        http::HeaderMap,
        response::Response,
        routing::post,
    };
    use tokio::net::TcpListener;

    use super::*;

    async fn parse_handler(
        State(token): State<Arc<String>>,
        headers: HeaderMap,
        body: Bytes,
    ) -> (StatusCode, String) {
        assert_eq!(
            headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()),
            Some(format!("Bearer {token}").as_str())
        );
        assert!(body.starts_with(b"--"));
        assert!(body.windows(5).any(|w| w == b"%PDF-"));
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            r#"{"status":"failed","parsed_document":null,"error_code":"ocr_required"}"#.to_owned(),
        )
    }

    #[tokio::test]
    async fn authenticated_parse_uses_fixed_path_and_safe_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let token = Arc::new("fictional-internal-token-1234567890".to_owned());
        let app = Router::new()
            .route("/internal/v1/parse", post(parse_handler))
            .with_state(token.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = HttpTenderAnalysisClient::new(
            format!("http://{address}/").parse().unwrap(),
            SecretValue::new(token.as_str()),
            Duration::from_secs(3),
        )
        .unwrap();
        assert!(matches!(
            client.parse(b"%PDF-fictional").await,
            Err(ClientError::OcrRequired)
        ));
        server.abort();
    }

    async fn oversized_response() -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_LENGTH, (MAX_RESPONSE_BYTES + 1).to_string())
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn response_cap_rejects_declared_oversize() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route("/internal/v1/parse", post(oversized_response));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = HttpTenderAnalysisClient::new(
            format!("http://{address}/").parse().unwrap(),
            SecretValue::new("fictional-internal-token-1234567890"),
            Duration::from_secs(3),
        )
        .unwrap();
        assert!(matches!(
            client.parse(b"%PDF-fictional").await,
            Err(ClientError::InvalidContract)
        ));
        server.abort();
    }

    async fn slow_response() -> StatusCode {
        tokio::time::sleep(Duration::from_millis(150)).await;
        StatusCode::OK
    }

    #[tokio::test]
    async fn request_timeout_is_safe_and_retryable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route("/internal/v1/parse", post(slow_response));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = HttpTenderAnalysisClient::new(
            format!("http://{address}/").parse().unwrap(),
            SecretValue::new("fictional-internal-token-1234567890"),
            Duration::from_millis(10),
        )
        .unwrap();
        let error = client.parse(b"%PDF-fictional").await.err().unwrap();
        assert!(error.is_retryable());
        assert_eq!(error.to_string(), "analysis_unavailable");
        server.abort();
    }
}
