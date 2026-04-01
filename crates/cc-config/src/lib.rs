//! Configuration management for Claude Code RS.
//!
//! Handles loading, merging, and validating configuration from
//! multiple sources: defaults, config files, environment variables,
//! and CLI flags.

use cc_error::{CcError, CcResult};
use cc_types::ModelId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// The model to use for conversations.
    pub model: ModelId,
    /// The API key for authentication.
    pub api_key: Option<String>,
    /// The base URL for the API.
    pub api_base_url: String,
    /// Maximum number of retries for transient errors.
    pub max_retries: u32,
    /// Timeout for API requests in seconds.
    pub request_timeout_secs: u64,
    /// Whether to enable verbose logging.
    pub verbose: bool,
    /// The directory for storing session data.
    pub data_dir: PathBuf,
    /// Permission configuration.
    pub permissions: PermissionsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("claude-code-rs");

        Self {
            model: ModelId::default(),
            api_key: None,
            api_base_url: "https://api.anthropic.com".to_string(),
            max_retries: 3,
            request_timeout_secs: 300,
            verbose: false,
            data_dir,
            permissions: PermissionsConfig::default(),
        }
    }
}

/// Permission-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionsConfig {
    /// Directories that tools are allowed to read from.
    pub allowed_read_dirs: Vec<PathBuf>,
    /// Directories that tools are allowed to write to.
    pub allowed_write_dirs: Vec<PathBuf>,
    /// Commands that are allowed to execute without confirmation.
    pub allowed_commands: Vec<String>,
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            allowed_read_dirs: vec![],
            allowed_write_dirs: vec![],
            allowed_commands: vec![],
        }
    }
}

impl AppConfig {
    /// Load configuration by merging sources in priority order:
    /// 1. Built-in defaults
    /// 2. Global config file (~/.config/claude-code-rs/config.toml)
    /// 3. Project config file (.claude-code-rs.toml)
    /// 4. Environment variables
    pub fn load() -> CcResult<Self> {
        let mut config = Self::default();

        // Try loading global config
        if let Some(global_path) = Self::global_config_path() {
            if global_path.exists() {
                config = Self::merge_from_file(config, &global_path)?;
            }
        }

        // Try loading project config
        let project_path = PathBuf::from(".claude-code-rs.toml");
        if project_path.exists() {
            config = Self::merge_from_file(config, &project_path)?;
        }

        // Override with environment variables
        config = Self::merge_from_env(config);

        Ok(config)
    }

    /// Get the path to the global configuration file.
    pub fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("claude-code-rs").join("config.toml"))
    }

    /// Merge configuration from a TOML file.
    fn merge_from_file(base: Self, path: &Path) -> CcResult<Self> {
        let content = std::fs::read_to_string(path).map_err(CcError::Io)?;
        let file_config: AppConfig =
            toml::from_str(&content).map_err(|e| CcError::Config(e.to_string()))?;
        tracing::debug!("Loaded config from {}", path.display());
        Ok(Self::merge(base, file_config))
    }

    /// Merge environment variable overrides.
    fn merge_from_env(mut config: Self) -> Self {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            config.api_key = Some(key);
        }
        if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
            config.api_base_url = url;
        }
        if let Ok(model) = std::env::var("CLAUDE_MODEL") {
            config.model = ModelId(model);
        }
        config
    }

    /// Merge two configs, preferring values from `overlay` when they
    /// differ from defaults.
    fn merge(base: Self, overlay: Self) -> Self {
        // Simple strategy: overlay wins for non-default values.
        // A more sophisticated approach could be added later.
        Self {
            model: overlay.model,
            api_key: overlay.api_key.or(base.api_key),
            api_base_url: overlay.api_base_url,
            max_retries: overlay.max_retries,
            request_timeout_secs: overlay.request_timeout_secs,
            verbose: overlay.verbose || base.verbose,
            data_dir: overlay.data_dir,
            permissions: overlay.permissions,
        }
    }

    /// Validate the configuration, returning errors for invalid values.
    pub fn validate(&self) -> CcResult<()> {
        if self.request_timeout_secs == 0 {
            return Err(CcError::Config(
                "request_timeout_secs must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}
