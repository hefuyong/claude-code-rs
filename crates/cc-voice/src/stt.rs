//! Speech-to-text client for voice mode.
//!
//! Provides both streaming (WebSocket) and batch (HTTP POST) transcription
//! against a configurable STT endpoint.

use async_stream::stream;
use cc_error::{CcError, CcResult};
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::audio::AudioStream;

/// Configuration for the STT client.
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// Base URL of the STT service.
    pub url: String,
    /// API key / bearer token.
    pub api_key: String,
    /// Language code (e.g. "en-US").
    pub language: String,
    /// Model identifier to use for transcription.
    pub model: String,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            url: "https://api.anthropic.com/v1/audio/transcriptions".to_string(),
            api_key: String::new(),
            language: "en-US".to_string(),
            model: "default".to_string(),
        }
    }
}

/// A transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    /// The transcribed text.
    pub text: String,
    /// Whether this is the final transcription for the utterance.
    pub is_final: bool,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// Duration of the transcribed audio in milliseconds.
    pub duration_ms: u64,
}

/// STT HTTP request body (batch mode).
#[derive(Debug, Serialize)]
struct TranscribeRequest {
    audio: String, // base64-encoded PCM
    language: String,
    model: String,
    encoding: String,
    sample_rate: u32,
}

/// STT HTTP response body (batch mode).
#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    duration_ms: u64,
}

/// Client for speech-to-text transcription.
pub struct SttClient {
    config: SttConfig,
    http: reqwest::Client,
}

impl SttClient {
    /// Create a new STT client.
    pub fn new(config: SttConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self { config, http }
    }

    /// Stream audio chunks to the STT service and yield transcript events.
    ///
    /// Each chunk from the [`AudioStream`] is sent to the service, and partial
    /// or final transcripts are yielded as they arrive.
    pub fn transcribe_stream(
        &self,
        mut audio: AudioStream,
    ) -> impl Stream<Item = CcResult<Transcript>> + '_ {
        stream! {
            // Accumulate chunks and periodically send for transcription.
            let mut buffer = Vec::new();
            let chunk_threshold = self.config_chunk_size();

            loop {
                match audio.next_chunk().await {
                    Some(chunk) => {
                        buffer.extend_from_slice(&chunk);

                        // When we have enough audio, send a batch request for
                        // an interim transcript.
                        if buffer.len() >= chunk_threshold {
                            match self.transcribe(&buffer).await {
                                Ok(transcript) => {
                                    yield Ok(Transcript {
                                        is_final: false,
                                        ..transcript
                                    });
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "interim transcription failed");
                                    // Non-fatal: continue accumulating.
                                }
                            }
                            buffer.clear();
                        }
                    }
                    None => {
                        // Stream ended; produce the final transcript.
                        if !buffer.is_empty() {
                            match self.transcribe(&buffer).await {
                                Ok(transcript) => {
                                    yield Ok(Transcript {
                                        is_final: true,
                                        ..transcript
                                    });
                                }
                                Err(e) => yield Err(e),
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Transcribe a complete audio buffer (non-streaming).
    pub async fn transcribe(&self, audio_data: &[u8]) -> CcResult<Transcript> {
        if audio_data.is_empty() {
            return Ok(Transcript {
                text: String::new(),
                is_final: true,
                confidence: 0.0,
                duration_ms: 0,
            });
        }

        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            audio_data,
        );

        let body = TranscribeRequest {
            audio: encoded,
            language: self.config.language.clone(),
            model: self.config.model.clone(),
            encoding: "pcm16".to_string(),
            sample_rate: 16_000,
        };

        let response = self
            .http
            .post(&self.config.url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: format!("STT request failed: {e}"),
                status_code: None,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(CcError::Api {
                message: format!("STT returned {status}: {body_text}"),
                status_code: Some(status.as_u16()),
            });
        }

        let resp: TranscribeResponse =
            response.json().await.map_err(|e| CcError::Serialization(e.to_string()))?;

        Ok(Transcript {
            text: resp.text,
            is_final: true,
            confidence: resp.confidence,
            duration_ms: resp.duration_ms,
        })
    }

    /// How many bytes of audio to accumulate before sending an interim request.
    /// At 16 kHz, 16-bit mono, 1 second = 32 000 bytes. We send every ~2 seconds.
    fn config_chunk_size(&self) -> usize {
        64_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stt_config() {
        let cfg = SttConfig::default();
        assert!(cfg.url.contains("anthropic.com"));
        assert_eq!(cfg.language, "en-US");
    }

    #[tokio::test]
    async fn transcribe_empty_audio() {
        let client = SttClient::new(SttConfig::default());
        let result = client.transcribe(&[]).await.unwrap();
        assert!(result.text.is_empty());
        assert!(result.is_final);
    }

    #[test]
    fn transcript_serialization_roundtrip() {
        let t = Transcript {
            text: "hello world".into(),
            is_final: true,
            confidence: 0.95,
            duration_ms: 1500,
        };
        let json = serde_json::to_string(&t).unwrap();
        let t2: Transcript = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.text, "hello world");
        assert_eq!(t2.confidence, 0.95);
    }
}
