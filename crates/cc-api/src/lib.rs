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
