//! Voice mode configuration.

use serde::{Deserialize, Serialize};

/// Default STT endpoint for Anthropic's speech service.
const DEFAULT_STT_URL: &str = "https://api.anthropic.com/v1/audio/transcriptions";

/// Default audio sample rate in Hz.
const DEFAULT_SAMPLE_RATE: u32 = 16_000;

/// Default number of audio channels (mono).
const DEFAULT_CHANNELS: u16 = 1;

/// Configuration for voice mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Whether voice mode is enabled.
    pub enabled: bool,
    /// URL of the speech-to-text service.
    pub stt_url: String,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Language code for STT (e.g. "en-US").
    pub language: String,
    /// Key binding for push-to-talk (e.g. "ctrl+shift+v").
    pub push_to_talk_key: Option<String>,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stt_url: DEFAULT_STT_URL.to_string(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            language: "en-US".to_string(),
            push_to_talk_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let cfg = VoiceConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.sample_rate, 16_000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.language, "en-US");
        assert!(cfg.stt_url.contains("anthropic.com"));
    }
}
