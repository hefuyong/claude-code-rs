//! Settings migration system for Claude Code RS.
//!
//! Provides a framework for versioned, incremental migrations that
//! transform a JSON settings blob from one schema version to the next.
//! Each migration is a pure function that receives a mutable
//! [`serde_json::Value`] and returns whether any changes were made.

use std::path::PathBuf;

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

/// A single migration step.
pub struct Migration {
    /// Unique, human-readable identifier (e.g. `"001_model_rename_fennec_to_opus"`).
    pub id: &'static str,
    /// Short description of what the migration does.
    pub description: &'static str,
    /// The minimum settings version this migration applies to.
    pub version_from: &'static str,
    /// The settings version after this migration has been applied.
    pub version_to: &'static str,
    /// The migration function.  Returns `Ok(true)` when it mutated `settings`.
    pub migrate: fn(&mut serde_json::Value) -> CcResult<bool>,
}

// ---------------------------------------------------------------------------
// Applied record (persisted to disk)
// ---------------------------------------------------------------------------

/// Record of migrations that have already been applied.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppliedRecord {
    applied: Vec<String>,
}

// ---------------------------------------------------------------------------
// MigrationRunner
// ---------------------------------------------------------------------------

/// Orchestrates discovery and execution of migrations.
pub struct MigrationRunner {
    migrations: Vec<Migration>,
    applied_file: PathBuf,
}

impl MigrationRunner {
    /// Create a runner that persists state to `~/.claude/migrations_applied.json`.
    pub fn new() -> CcResult<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| CcError::Config("cannot determine home directory".into()))?;
        let applied_file = home.join(".claude").join("migrations_applied.json");
        Ok(Self {
            migrations: Vec::new(),
            applied_file,
        })
    }

    /// Create a runner that uses a custom path for the applied-migrations file.
    /// Useful for testing.
    pub fn with_applied_file(applied_file: PathBuf) -> Self {
        Self {
            migrations: Vec::new(),
            applied_file,
        }
    }

    /// Register a single migration.
    pub fn register(&mut self, migration: Migration) {
        self.migrations.push(migration);
    }

    /// Register all built-in migrations provided by [`builtin_migrations`].
    pub fn register_all_builtin(&mut self) {
        for m in builtin_migrations() {
            self.migrations.push(m);
        }
    }

    /// Run all pending migrations on `settings`, returning the IDs of
    /// those that were actually applied (i.e. that mutated the value).
    pub fn run(&self, settings: &mut serde_json::Value) -> CcResult<Vec<String>> {
        let already_applied = self.applied()?;
        let mut newly_applied = Vec::new();

        for migration in &self.migrations {
            if already_applied.contains(&migration.id.to_string()) {
                continue;
            }

            tracing::debug!(id = migration.id, desc = migration.description, "running migration");

            match (migration.migrate)(settings) {
                Ok(true) => {
                    tracing::info!(id = migration.id, "migration applied");
                    self.mark_applied(migration.id)?;
                    newly_applied.push(migration.id.to_string());
                }
                Ok(false) => {
                    // Migration decided nothing needed changing -- still mark
                    // it so we don't re-evaluate it next time.
                    self.mark_applied(migration.id)?;
                }
                Err(e) => {
                    tracing::error!(id = migration.id, error = %e, "migration failed");
                    return Err(e);
                }
            }
        }

        Ok(newly_applied)
    }

    /// Return references to migrations that have not yet been applied.
    pub fn pending(&self) -> CcResult<Vec<&Migration>> {
        let already = self.applied()?;
        Ok(self
            .migrations
            .iter()
            .filter(|m| !already.contains(&m.id.to_string()))
            .collect())
    }

    /// Persist the fact that `id` has been applied.
    pub fn mark_applied(&self, id: &str) -> CcResult<()> {
        let mut record = self.load_record();
        if !record.applied.contains(&id.to_string()) {
            record.applied.push(id.to_string());
        }
        self.save_record(&record)
    }

    /// Return the list of already-applied migration IDs.
    pub fn applied(&self) -> CcResult<Vec<String>> {
        Ok(self.load_record().applied)
    }

    // -- internal helpers ---------------------------------------------------

    fn load_record(&self) -> AppliedRecord {
        std::fs::read_to_string(&self.applied_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_record(&self, record: &AppliedRecord) -> CcResult<()> {
        if let Some(parent) = self.applied_file.parent() {
            std::fs::create_dir_all(parent).map_err(CcError::Io)?;
        }
        let json = serde_json::to_string_pretty(record)
            .map_err(|e| CcError::Serialization(e.to_string()))?;
        std::fs::write(&self.applied_file, json).map_err(CcError::Io)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Built-in migrations
// ---------------------------------------------------------------------------

/// Return the full list of built-in migrations shipped with the application.
pub fn builtin_migrations() -> Vec<Migration> {
    vec![
        Migration {
            id: "001_model_rename_fennec_to_opus",
            description: "Rename model 'fennec' to 'opus'",
            version_from: "0.0.0",
            version_to: "0.1.0",
            migrate: |settings| {
                if let Some(model) = settings.get_mut("model") {
                    if model.as_str() == Some("fennec") {
                        *model =
                            serde_json::Value::String("claude-opus-4-20250514".into());
                        return Ok(true);
                    }
                }
                Ok(false)
            },
        },
        Migration {
            id: "002_sonnet_45_to_46",
            description: "Update sonnet 4.5 to 4.6",
            version_from: "0.1.0",
            version_to: "0.1.1",
            migrate: |settings| {
                if let Some(model) = settings.get_mut("model") {
                    if model.as_str() == Some("claude-sonnet-4-5-20250514") {
                        *model =
                            serde_json::Value::String("claude-sonnet-4-6-20250514".into());
                        return Ok(true);
                    }
                }
                Ok(false)
            },
        },
        Migration {
            id: "003_add_permissions_structure",
            description: "Add permissions object if missing",
            version_from: "0.1.0",
            version_to: "0.1.1",
            migrate: |settings| {
                if settings.get("permissions").is_none() {
                    settings["permissions"] = serde_json::json!({
                        "allow": [],
                        "deny": []
                    });
                    return Ok(true);
                }
                Ok(false)
            },
        },
        Migration {
            id: "004_add_hooks_structure",
            description: "Add hooks configuration if missing",
            version_from: "0.1.0",
            version_to: "0.1.1",
            migrate: |settings| {
                if settings.get("hooks").is_none() {
                    settings["hooks"] = serde_json::json!({});
                    return Ok(true);
                }
                Ok(false)
            },
        },
        Migration {
            id: "005_rename_api_key_field",
            description: "Rename 'apiKey' to 'api_key' for consistency",
            version_from: "0.1.1",
            version_to: "0.1.2",
            migrate: |settings| {
                if let Some(val) = settings.get("apiKey").cloned() {
                    if settings.get("api_key").is_none() {
                        settings["api_key"] = val;
                    }
                    if let Some(obj) = settings.as_object_mut() {
                        obj.remove("apiKey");
                    }
                    return Ok(true);
                }
                Ok(false)
            },
        },
        Migration {
            id: "006_add_mcp_servers_array",
            description: "Ensure 'mcpServers' key exists as an object",
            version_from: "0.1.2",
            version_to: "0.1.3",
            migrate: |settings| {
                if settings.get("mcpServers").is_none() {
                    settings["mcpServers"] = serde_json::json!({});
                    return Ok(true);
                }
                Ok(false)
            },
        },
        Migration {
            id: "007_add_output_styles_default",
            description: "Add default output style 'concise' if not set",
            version_from: "0.1.3",
            version_to: "0.1.4",
            migrate: |settings| {
                if settings.get("outputStyle").is_none() {
                    settings["outputStyle"] =
                        serde_json::Value::String("concise".into());
                    return Ok(true);
                }
                Ok(false)
            },
        },
        Migration {
            id: "008_add_feature_flags_object",
            description: "Add feature_flags object if missing",
            version_from: "0.1.4",
            version_to: "0.1.5",
            migrate: |settings| {
                if settings.get("feature_flags").is_none() {
                    settings["feature_flags"] = serde_json::json!({});
                    return Ok(true);
                }
                Ok(false)
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runner(dir: &std::path::Path) -> MigrationRunner {
        MigrationRunner::with_applied_file(dir.join("migrations_applied.json"))
    }

    #[test]
    fn test_builtin_migrations_count() {
        let migrations = builtin_migrations();
        assert!(migrations.len() >= 8, "expected at least 8 built-in migrations");
    }

    #[test]
    fn test_fennec_to_opus_migration() {
        let mut settings = serde_json::json!({ "model": "fennec" });
        let migrations = builtin_migrations();
        let m = &migrations[0];
        let changed = (m.migrate)(&mut settings).unwrap();
        assert!(changed);
        assert_eq!(settings["model"], "claude-opus-4-20250514");
    }

    #[test]
    fn test_fennec_migration_noop_on_other_model() {
        let mut settings = serde_json::json!({ "model": "haiku" });
        let migrations = builtin_migrations();
        let changed = (migrations[0].migrate)(&mut settings).unwrap();
        assert!(!changed);
        assert_eq!(settings["model"], "haiku");
    }

    #[test]
    fn test_permissions_migration_adds_structure() {
        let mut settings = serde_json::json!({});
        let migrations = builtin_migrations();
        let changed = (migrations[2].migrate)(&mut settings).unwrap();
        assert!(changed);
        assert!(settings.get("permissions").is_some());
        assert_eq!(settings["permissions"]["allow"], serde_json::json!([]));
    }

    #[test]
    fn test_permissions_migration_noop_when_exists() {
        let mut settings = serde_json::json!({ "permissions": { "allow": ["bash"] } });
        let migrations = builtin_migrations();
        let changed = (migrations[2].migrate)(&mut settings).unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_runner_run_all_on_empty_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let mut runner = test_runner(tmp.path());
        runner.register_all_builtin();

        let mut settings = serde_json::json!({});
        let applied = runner.run(&mut settings).unwrap();

        // Several migrations should have fired (those that add missing keys).
        assert!(!applied.is_empty());
        assert!(settings.get("permissions").is_some());
        assert!(settings.get("hooks").is_some());
        assert!(settings.get("mcpServers").is_some());
    }

    #[test]
    fn test_runner_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut runner = test_runner(tmp.path());
        runner.register_all_builtin();

        let mut settings = serde_json::json!({});
        let first_run = runner.run(&mut settings).unwrap();
        assert!(!first_run.is_empty());

        // Second run should produce zero newly-applied migrations.
        let second_run = runner.run(&mut settings).unwrap();
        assert!(second_run.is_empty());
    }

    #[test]
    fn test_runner_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let mut runner = test_runner(tmp.path());
        runner.register_all_builtin();

        let pending_before = runner.pending().unwrap();
        assert_eq!(pending_before.len(), runner.migrations.len());

        let mut settings = serde_json::json!({});
        runner.run(&mut settings).unwrap();

        let pending_after = runner.pending().unwrap();
        assert!(pending_after.is_empty());
    }

    #[test]
    fn test_rename_api_key_migration() {
        let mut settings = serde_json::json!({ "apiKey": "sk-test-123" });
        let migrations = builtin_migrations();
        // migration index 4 = 005_rename_api_key_field
        let changed = (migrations[4].migrate)(&mut settings).unwrap();
        assert!(changed);
        assert_eq!(settings["api_key"], "sk-test-123");
        assert!(settings.get("apiKey").is_none());
    }
}
