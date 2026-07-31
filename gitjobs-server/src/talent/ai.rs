use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{config::TalentAiConfig, talent::TalentExtraction};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_OUTPUT_TOKENS: u16 = 2_048;
pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const SYSTEM_PROMPT: &str = "talent-extract-v1: Extract structured talent information from the supplied text. Return a JSON object matching the requested schema. Use null when uncertain. Never claim verification or truth.";

pub(crate) struct ExtractionRequest {
    pub fragment: String,
}

impl ExtractionRequest {
    pub(crate) fn new(fragment: impl Into<String>) -> Self {
        Self {
            fragment: fragment.into(),
        }
    }
}

#[async_trait]
pub(crate) trait TalentAiProvider: Send + Sync {
    async fn extract(&self, request: ExtractionRequest) -> Result<TalentExtraction, AiError>;
}

pub(crate) struct OpenAiCompatibleProvider {
    config: TalentAiConfig,
    client: reqwest::Client,
    request_timeout: Duration,
}

impl OpenAiCompatibleProvider {
    pub(crate) fn new(config: TalentAiConfig, client: reqwest::Client) -> Self {
        Self {
            config,
            client,
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(config: TalentAiConfig, client: reqwest::Client, request_timeout: Duration) -> Self {
        Self {
            config,
            client,
            request_timeout,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum AiError {
    #[error("AI provider request timed out")]
    Timeout,
    #[error("AI provider request failed")]
    Transport,
    #[error("AI provider returned HTTP status {0}")]
    HttpStatus(StatusCode),
    #[error("AI provider returned an invalid response")]
    InvalidResponse,
    #[error("AI provider response exceeds the allowed size")]
    ResponseTooLarge,
    #[error("AI provider response has no usable choice")]
    MissingChoice,
    #[error("AI provider choice content is not JSON")]
    InvalidJson,
    #[error("AI provider choice does not match the extraction schema")]
    InvalidSchema,
}

#[derive(Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    max_tokens: u16,
    messages: [ChatMessage<'a>; 2],
    temperature: u8,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    json_schema: serde_json::Value,
}
fn extraction_response_format() -> ResponseFormat {
    ResponseFormat {
        kind: "json_schema",
        json_schema: serde_json::json!({
            "name": "talent_extraction_v1",
            "strict": true,
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "kind", "confidence", "title", "scene", "region", "city", "district",
                    "salaryMin", "salaryMax", "roles", "skills", "experienceYears",
                    "hoursPerDay", "restDaysPerMonth", "lodging", "housingAllowance",
                    "availability", "urgency", "publishedAt", "contacts", "missingFields",
                    "riskCandidates"
                ],
                "properties": {
                    "kind": {"type": "string", "enum": ["hiring", "seeking", "irrelevant", "uncertain"]},
                    "confidence": {"type": "number", "minimum": 0, "maximum": 1},
                    "title": {"type": ["string", "null"]},
                    "scene": {"type": ["string", "null"]},
                    "region": {"type": ["string", "null"]},
                    "city": {"type": ["string", "null"]},
                    "district": {"type": ["string", "null"]},
                    "salaryMin": {"type": ["integer", "null"], "minimum": 0},
                    "salaryMax": {"type": ["integer", "null"], "minimum": 0},
                    "roles": {"type": "array", "maxItems": 50, "items": {"type": "string"}},
                    "skills": {"type": "array", "maxItems": 50, "items": {"type": "string"}},
                    "experienceYears": {"type": ["integer", "null"], "minimum": 0, "maximum": 255},
                    "hoursPerDay": {"type": ["integer", "null"], "minimum": 0, "maximum": 255},
                    "restDaysPerMonth": {"type": ["integer", "null"], "minimum": 0, "maximum": 255},
                    "lodging": {"type": ["string", "null"]},
                    "housingAllowance": {"type": ["integer", "null"], "minimum": 0},
                    "availability": {"type": ["string", "null"]},
                    "urgency": {"type": ["string", "null"]},
                    "publishedAt": {"type": ["string", "null"], "format": "date-time"},
                    "contacts": {
                        "type": "array",
                        "maxItems": 50,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["contact_type", "value"],
                            "properties": {
                                "contact_type": {"type": "string", "enum": ["phone", "wechat", "qq", "other"]},
                                "value": {"type": "string"}
                            }
                        }
                    },
                    "missingFields": {"type": "array", "maxItems": 50, "items": {"type": "string"}},
                    "riskCandidates": {
                        "type": "array",
                        "maxItems": 50,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["code", "explanation", "evidence", "hard_block"],
                            "properties": {
                                "code": {"type": "string"},
                                "explanation": {"type": "string"},
                                "evidence": {"type": ["string", "null"]},
                                "hard_block": {"type": "boolean"}
                            }
                        }
                    }
                }
            }
        }),
    }
}

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
}

#[derive(Deserialize)]
struct CompletionMessage {
    content: String,
}

#[async_trait]
impl TalentAiProvider for OpenAiCompatibleProvider {
    async fn extract(&self, request: ExtractionRequest) -> Result<TalentExtraction, AiError> {
        tokio::time::timeout(self.request_timeout, async {
            let endpoint = self
                .config
                .base_url
                .join("chat/completions")
                .map_err(|_| AiError::Transport)?;
            let payload = CompletionRequest {
                model: &self.config.model,
                messages: [
                    ChatMessage {
                        role: "system",
                        content: SYSTEM_PROMPT,
                    },
                    ChatMessage {
                        role: "user",
                        content: &request.fragment,
                    },
                ],
                temperature: 0,
                max_tokens: MAX_OUTPUT_TOKENS,
                response_format: extraction_response_format(),
            };
            let mut response = self
                .client
                .post(endpoint)
                .bearer_auth(self.config.api_key.expose_secret())
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        AiError::Timeout
                    } else {
                        AiError::Transport
                    }
                })?;
            if !response.status().is_success() {
                return Err(AiError::HttpStatus(response.status()));
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|error| {
                if error.is_timeout() {
                    AiError::Timeout
                } else {
                    AiError::InvalidResponse
                }
            })? {
                if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return Err(AiError::ResponseTooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            let completion: CompletionResponse =
                serde_json::from_slice(&body).map_err(|_| AiError::InvalidResponse)?;
            let content = completion
                .choices
                .into_iter()
                .next()
                .map(|choice| choice.message.content)
                .filter(|content| !content.trim().is_empty())
                .ok_or(AiError::MissingChoice)?;
            let value: serde_json::Value =
                serde_json::from_str(&content).map_err(|_| AiError::InvalidJson)?;
            serde_json::from_value(value).map_err(|_| AiError::InvalidSchema)
        })
        .await
        .unwrap_or(Err(AiError::Timeout))
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, future::IntoFuture, sync::Arc, time::Duration};

    use axum::{
        Json, Router,
        body::{Body, Bytes},
        extract::State,
        http::{HeaderMap, Response, StatusCode},
        routing::post,
    };
    use tokio::net::TcpListener;

    use crate::{
        config::{SecretValue, TalentAiConfig},
        talent::{ContactCandidate, ContactType, TalentExtraction, TalentKind},
    };

    use super::{AiError, ExtractionRequest, MAX_RESPONSE_BYTES, OpenAiCompatibleProvider, TalentAiProvider};

    #[derive(Clone)]
    struct MockResponse {
        status: StatusCode,
        body: String,
    }

    async fn mock_completion(
        State(response): State<Arc<MockResponse>>,
        headers: HeaderMap,
        Json(payload): Json<serde_json::Value>,
    ) -> (StatusCode, String) {
        assert_eq!(headers.get("authorization").unwrap(), "Bearer test-api-key");
        assert_eq!(payload["model"], "test-model");
        assert_eq!(payload["temperature"], 0);
        assert_eq!(payload["max_tokens"], 2_048);
        assert_eq!(payload["response_format"]["type"], "json_schema");
        assert_eq!(
            payload["response_format"]["json_schema"]["name"],
            "talent_extraction_v1"
        );
        assert_eq!(payload["response_format"]["json_schema"]["strict"], true);
        let schema = &payload["response_format"]["json_schema"]["schema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "contacts")
        );
        assert_eq!(
            schema["properties"]["contacts"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(
            schema["properties"]["riskCandidates"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(
            schema["properties"]["kind"]["enum"],
            serde_json::json!(["hiring", "seeking", "irrelevant", "uncertain"])
        );
        assert!(
            payload["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("talent-extract-v1")
        );
        (response.status, response.body.clone())
    }

    async fn provider(response: MockResponse) -> OpenAiCompatibleProvider {
        let app = Router::new()
            .route("/v1/chat/completions", post(mock_completion))
            .with_state(Arc::new(response));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).into_future().await.unwrap();
        });
        OpenAiCompatibleProvider::new(
            TalentAiConfig {
                base_url: reqwest::Url::parse(&format!("http://{address}/v1/")).unwrap(),
                model: "test-model".to_owned(),
                api_key: SecretValue::new("test-api-key"),
            },
            reqwest::Client::new(),
        )
    }

    async fn stalled_completion_body() -> Response<Body> {
        let body = Body::from_stream(futures::stream::pending::<Result<Bytes, Infallible>>());
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    }

    async fn oversized_chunked_completion_body() -> Response<Body> {
        let chunks = [
            Ok::<_, Infallible>(Bytes::from(vec![b'a'; MAX_RESPONSE_BYTES / 2 + 1])),
            Ok::<_, Infallible>(Bytes::from(vec![b'b'; MAX_RESPONSE_BYTES / 2 + 1])),
        ];
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from_stream(futures::stream::iter(chunks)))
            .unwrap()
    }

    #[tokio::test]
    async fn provider_deadline_includes_reading_the_response_body() {
        let app = Router::new().route("/v1/chat/completions", post(stalled_completion_body));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).into_future().await.unwrap();
        });
        let provider = OpenAiCompatibleProvider::with_timeout(
            TalentAiConfig {
                base_url: reqwest::Url::parse(&format!("http://{address}/v1/")).unwrap(),
                model: "test-model".to_owned(),
                api_key: SecretValue::new("test-api-key"),
            },
            reqwest::Client::new(),
            Duration::from_millis(25),
        );

        let result = tokio::time::timeout(
            Duration::from_millis(250),
            provider.extract(ExtractionRequest::new("candidate text")),
        )
        .await
        .expect("provider should enforce its own deadline");

        assert!(matches!(result, Err(AiError::Timeout)));
    }

    #[tokio::test]
    async fn provider_rejects_chunked_responses_above_the_strict_byte_limit() {
        let app = Router::new().route("/v1/chat/completions", post(oversized_chunked_completion_body));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).into_future().await.unwrap();
        });
        let provider = OpenAiCompatibleProvider::new(
            TalentAiConfig {
                base_url: reqwest::Url::parse(&format!("http://{address}/v1/")).unwrap(),
                model: "test-model".to_owned(),
                api_key: SecretValue::new("test-api-key"),
            },
            reqwest::Client::new(),
        );

        let error = provider
            .extract(ExtractionRequest::new("candidate text"))
            .await
            .unwrap_err();

        assert!(matches!(error, AiError::ResponseTooLarge));
        assert!(!error.to_string().contains("aaaaaaaaaaaaaaaa"));
    }

    fn valid_extraction() -> TalentExtraction {
        TalentExtraction {
            kind: TalentKind::Hiring,
            confidence: 0.8,
            title: Some("Developer".to_owned()),
            roles: vec!["Engineer".to_owned()],
            skills: vec!["Rust".to_owned()],
            contacts: vec![ContactCandidate {
                contact_type: ContactType::Wechat,
                value: "candidate_01".to_owned(),
            }],
            ..TalentExtraction::default()
        }
    }

    fn completion_for(extraction: TalentExtraction) -> String {
        serde_json::json!({"choices": [{"message": {"content": serde_json::to_string(&extraction).unwrap()}}]}).to_string()
    }

    #[tokio::test]
    async fn provider_parses_a_strict_extraction_from_the_first_choice() {
        let provider = provider(MockResponse {
            status: StatusCode::OK,
            body: completion_for(valid_extraction()),
        })
        .await;

        assert_eq!(
            provider
                .extract(ExtractionRequest::new("candidate text"))
                .await
                .unwrap(),
            valid_extraction()
        );
    }

    #[tokio::test]
    async fn provider_rejects_unknown_extraction_fields() {
        let provider = provider(MockResponse {
            status: StatusCode::OK,
            body: serde_json::json!({"choices": [{"message": {"content": r#"{"kind":"uncertain","confidence":0.0,"roles":[],"skills":[],"contacts":[],"missingFields":[],"riskCandidates":[],"unexpected":true}"#}}]}).to_string(),
        })
        .await;

        assert!(matches!(
            provider.extract(ExtractionRequest::new("candidate text")).await,
            Err(AiError::InvalidSchema)
        ));
    }

    #[tokio::test]
    async fn provider_reports_missing_choice_without_exposing_response_content() {
        let body = r#"{"choices":[]}"#;
        let provider = provider(MockResponse {
            status: StatusCode::OK,
            body: body.to_owned(),
        })
        .await;
        let error = provider
            .extract(ExtractionRequest::new("candidate text"))
            .await
            .unwrap_err();

        assert!(matches!(error, AiError::MissingChoice));
        assert!(!error.to_string().contains(body));
    }

    #[tokio::test]
    async fn provider_reports_non_json_content_without_exposing_it() {
        let content = "not json; definitely not a response body";
        let provider = provider(MockResponse {
            status: StatusCode::OK,
            body: serde_json::json!({"choices": [{"message": {"content": content}}]}).to_string(),
        })
        .await;
        let error = provider
            .extract(ExtractionRequest::new("candidate text"))
            .await
            .unwrap_err();

        assert!(matches!(error, AiError::InvalidJson));
        assert!(!error.to_string().contains(content));
    }

    #[tokio::test]
    async fn provider_redacts_http_response_bodies_and_api_keys_from_errors() {
        let provider = provider(MockResponse {
            status: StatusCode::BAD_REQUEST,
            body: "provider-secret-response".to_owned(),
        })
        .await;
        let error = provider
            .extract(ExtractionRequest::new("candidate text"))
            .await
            .unwrap_err();
        let displayed = error.to_string();

        assert!(matches!(error, AiError::HttpStatus(StatusCode::BAD_REQUEST)));
        assert!(!displayed.contains("provider-secret-response"));
        assert!(!displayed.contains("test-api-key"));
    }

    #[test]
    fn provider_config_redacts_its_secret_in_debug_output() {
        let config = TalentAiConfig {
            base_url: reqwest::Url::parse("https://api.example.invalid/v1/").unwrap(),
            model: "test-model".to_owned(),
            api_key: SecretValue::new("test-api-key"),
        };

        assert!(format!("{config:?}").contains("[REDACTED]"));
        assert!(!format!("{config:?}").contains("test-api-key"));
    }
}
