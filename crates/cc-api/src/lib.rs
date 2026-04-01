//! Claude API client with streaming SSE support and retry logic.

pub mod sse;
pub mod streaming;
pub mod types;

use cc_error::{CcError, CcResult};
use cc_types::{ModelId, ToolDefinition};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing;

/// Configuration for the API client.
#[derive(Debug, Clone)]
pub struct ApiClientConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: ModelId,
    pub max_retries: u32,
    pub request_timeout: Duration,
    pub max_tokens: u32,
}

impl Default for ApiClientConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".into(),
            model: ModelId::default(),
            max_retries: 3,
            request_timeout: Duration::from_secs(300),
            max_tokens: 16384,
        }
    }
}

/// The Anthropic Claude API client.
#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    config: ApiClientConfig,
}

impl ApiClient {
    /// Create a new API client with the given configuration.
    pub fn new(config: ApiClientConfig) -> CcResult<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&config.api_key)
                .map_err(|e| CcError::Config(format!("invalid API key: {e}")))?,
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static("2023-06-01"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| CcError::Internal(format!("failed to build HTTP client: {e}")))?;

        Ok(Self { http, config })
    }

    /// Send a messages request and get a streaming response.
    pub async fn send_streaming(
        &self,
        request: &CreateMessageRequest,
    ) -> CcResult<impl tokio_stream::Stream<Item = CcResult<types::StreamEvent>>> {
        let url = format!("{}/v1/messages", self.config.base_url);

        let mut body = serde_json::to_value(request)
            .map_err(|e| CcError::Serialization(e.to_string()))?;
        body["stream"] = serde_json::Value::Bool(true);

        let mut last_error: Option<CcError> = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.retry_delay(attempt, &last_error);
                tracing::info!(attempt, delay_ms = delay.as_millis(), "retrying API request");
                tokio::time::sleep(delay).await;
            }

            match self.do_streaming_request(&url, &body).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if e.is_retryable() && attempt < self.config.max_retries {
                        tracing::warn!(attempt, error = %e, "retryable API error");
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CcError::Internal("exhausted retries".into())))
    }

    /// Send a non-streaming request.
    pub async fn send(&self, request: &CreateMessageRequest) -> CcResult<types::MessageResponse> {
        let url = format!("{}/v1/messages", self.config.base_url);
        let body = serde_json::to_value(request)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: e.to_string(),
                status_code: e.status().map(|s| s.as_u16()),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.unwrap_or_default();
            return Err(self.map_api_error(status, &body_text));
        }

        let msg: types::MessageResponse = response
            .json()
            .await
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        Ok(msg)
    }

    async fn do_streaming_request(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> CcResult<impl tokio_stream::Stream<Item = CcResult<types::StreamEvent>>> {
        let response = self
            .http
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: e.to_string(),
                status_code: e.status().map(|s| s.as_u16()),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.unwrap_or_default();
            return Err(self.map_api_error(status, &body_text));
        }

        Ok(sse::parse_sse_stream(response))
    }

    fn retry_delay(&self, attempt: u32, last_error: &Option<CcError>) -> Duration {
        // Check for Retry-After from rate limit
        if let Some(CcError::RateLimited {
            retry_after_secs: Some(secs),
        }) = last_error
        {
            return Duration::from_secs(*secs);
        }
        // Exponential backoff: 1s, 2s, 4s, 8s...
        let base_ms = 1000u64 * 2u64.pow(attempt.saturating_sub(1));
        Duration::from_millis(base_ms)
    }

    fn map_api_error(&self, status: u16, body: &str) -> CcError {
        // Try to parse the error body
        let msg = serde_json::from_str::<types::ApiErrorBody>(body)
            .map(|e| e.error.message)
            .unwrap_or_else(|_| body.to_string());

        match status {
            429 => CcError::RateLimited {
                retry_after_secs: None,
            },
            401 => CcError::Auth(msg),
            400 if msg.contains("prompt is too long") => CcError::Api {
                message: format!("prompt too long: {msg}"),
                status_code: Some(400),
            },
            _ => CcError::Api {
                message: msg,
                status_code: Some(status),
            },
        }
    }
}

/// Request body for the Messages API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub model: String,
    pub messages: Vec<types::ApiMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl CreateMessageRequest {
    /// Create a simple request with one user message.
    pub fn simple(model: &ModelId, prompt: &str, max_tokens: u32) -> Self {
        Self {
            model: model.0.clone(),
            messages: vec![types::ApiMessage {
                role: "user".into(),
                content: types::ApiContent::Text(prompt.into()),
            }],
            max_tokens,
            system: None,
            tools: None,
            temperature: None,
            stop_sequences: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper: create an ApiClient with a dummy key for unit testing.
    fn test_client() -> ApiClient {
        ApiClient::new(ApiClientConfig {
            api_key: "test-key-for-unit-tests".to_string(),
            ..Default::default()
        })
        .expect("should build test client")
    }

    #[test]
    fn test_retry_delay_exponential() {
        let client = test_client();
        let no_error: Option<CcError> = None;

        // attempt 1 => 1s (2^0 * 1000ms)
        let d1 = client.retry_delay(1, &no_error);
        assert_eq!(d1, Duration::from_millis(1000));

        // attempt 2 => 2s (2^1 * 1000ms)
        let d2 = client.retry_delay(2, &no_error);
        assert_eq!(d2, Duration::from_millis(2000));

        // attempt 3 => 4s (2^2 * 1000ms)
        let d3 = client.retry_delay(3, &no_error);
        assert_eq!(d3, Duration::from_millis(4000));

        // Verify exponential doubling
        assert_eq!(d2.as_millis(), d1.as_millis() * 2);
        assert_eq!(d3.as_millis(), d2.as_millis() * 2);
    }

    #[test]
    fn test_retry_delay_rate_limited() {
        let client = test_client();
        let rate_limited = Some(CcError::RateLimited {
            retry_after_secs: Some(30),
        });

        // When rate limited with retry_after, should use that value
        let delay = client.retry_delay(1, &rate_limited);
        assert_eq!(delay, Duration::from_secs(30));

        // Even on a later attempt, retry_after takes precedence
        let delay2 = client.retry_delay(5, &rate_limited);
        assert_eq!(delay2, Duration::from_secs(30));
    }

    #[test]
    fn test_map_api_error_429() {
        let client = test_client();
        let err = client.map_api_error(429, "rate limited");
        assert!(
            matches!(err, CcError::RateLimited { .. }),
            "429 should map to RateLimited, got {:?}",
            err
        );
    }

    #[test]
    fn test_map_api_error_401() {
        let client = test_client();
        let err = client.map_api_error(401, "invalid api key");
        assert!(
            matches!(err, CcError::Auth(_)),
            "401 should map to Auth, got {:?}",
            err
        );
    }

    #[test]
    fn test_simple_request_creation() {
        let model = ModelId("claude-sonnet-4-20250514".to_string());
        let req = CreateMessageRequest::simple(&model, "Hello Claude", 1024);

        assert_eq!(req.model, "claude-sonnet-4-20250514");
        assert_eq!(req.max_tokens, 1024);
        assert!(req.system.is_none());
        assert!(req.tools.is_none());
        assert!(req.temperature.is_none());
        assert!(req.stop_sequences.is_none());
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        match &req.messages[0].content {
            types::ApiContent::Text(t) => assert_eq!(t, "Hello Claude"),
            other => panic!("expected Text content, got {:?}", other),
        }
    }
}
