use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{config::TalentAiConfig, talent::TalentExtraction};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
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
}

impl OpenAiCompatibleProvider {
    pub(crate) fn new(config: TalentAiConfig, client: reqwest::Client) -> Self {
        Self { config, client }
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
        let endpoint = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
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
            response_format: ResponseFormat { kind: "json_object" },
        };
        let send = self
            .client
            .post(endpoint)
            .bearer_auth(self.config.api_key.expose_secret())
            .json(&payload)
            .send();
        let response = match tokio::time::timeout(REQUEST_TIMEOUT, send).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) if error.is_timeout() => return Err(AiError::Timeout),
            Ok(Err(_)) => return Err(AiError::Transport),
            Err(_) => return Err(AiError::Timeout),
        };
        if !response.status().is_success() {
            return Err(AiError::HttpStatus(response.status()));
        }
        let completion: CompletionResponse = response.json().await.map_err(|_| AiError::InvalidResponse)?;
        let content = completion
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or(AiError::MissingChoice)?;
        let value: serde_json::Value = serde_json::from_str(&content).map_err(|_| AiError::InvalidJson)?;
        serde_json::from_value(value).map_err(|_| AiError::InvalidSchema)
    }
}

#[cfg(test)]
mod tests {
    use std::{future::IntoFuture, sync::Arc};

    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    use tokio::net::TcpListener;

    use crate::{
        config::{SecretValue, TalentAiConfig},
        talent::{ContactCandidate, ContactType, TalentExtraction, TalentKind},
    };

    use super::{AiError, ExtractionRequest, OpenAiCompatibleProvider, TalentAiProvider};

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
        assert_eq!(
            payload["response_format"],
            serde_json::json!({"type": "json_object"})
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
                base_url: format!("http://{address}/v1"),
                model: "test-model".to_owned(),
                api_key: SecretValue::new("test-api-key"),
            },
            reqwest::Client::new(),
        )
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
            base_url: "https://api.example.invalid/v1".to_owned(),
            model: "test-model".to_owned(),
            api_key: SecretValue::new("test-api-key"),
        };

        assert!(format!("{config:?}").contains("[REDACTED]"));
        assert!(!format!("{config:?}").contains("test-api-key"));
    }
}
