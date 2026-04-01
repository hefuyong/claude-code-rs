//! GlobTool -- find files matching a glob pattern.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::{Path, PathBuf};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g. \"**/*.rs\")"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to working directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("glob", "Missing required field: pattern"))?;

        let search_dir = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => {
                let path = Path::new(p);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    ctx.working_directory.join(path)
                }
            }
            None => ctx.working_directory.clone(),
        };

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| CcError::tool("glob", format!("Invalid pattern: {}", e)))?
            .compile_matcher();

        // Walk the directory tree synchronously in a blocking task.
        let matches = tokio::task::spawn_blocking(move || {
            let mut results: Vec<PathBuf> = Vec::new();
            walk_dir(&search_dir, &search_dir, &glob, &mut results);
            results.sort();
            results
        })
        .await
        .map_err(|e| CcError::Internal(format!("Join error: {}", e)))?;

        if matches.is_empty() {
            return Ok(ToolOutput::success("No files matched the pattern."));
        }

        let output = matches
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::success(format!(
            "{}\n\n({} files matched)",
            output,
            matches.len()
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}

/// Recursively walk `dir`, testing each file's path relative to `root`
/// against the glob matcher.
fn walk_dir(
    dir: &Path,
    root: &Path,
    glob: &globset::GlobMatcher,
    results: &mut Vec<PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories (starting with '.') for performance.
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            walk_dir(&path, root, glob, results);
        } else if let Ok(rel) = path.strip_prefix(root) {
            // Convert to forward slashes for cross-platform glob matching.
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if glob.is_match(&rel_str) {
                results.push(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_permissions::{PermissionContext, PermissionMode};
    use tempfile::TempDir;

    fn make_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_directory: dir.path().to_path_buf(),
            permission_context: PermissionContext::new(PermissionMode::Bypass, vec![]),
        }
    }

    #[tokio::test]
    async fn test_glob_finds_files() {
        let tmp = TempDir::new().unwrap();
        // Create some files to match.
        std::fs::write(tmp.path().join("one.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("two.rs"), "fn test() {}").unwrap();
        std::fs::write(tmp.path().join("readme.md"), "# README").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("three.rs"), "mod sub;").unwrap();

        let tool = GlobTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "pattern": "**/*.rs"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("one.rs"));
        assert!(output.content.contains("two.rs"));
        assert!(output.content.contains("three.rs"));
        assert!(!output.content.contains("readme.md"));
        assert!(output.content.contains("3 files matched"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("file.txt"), "content").unwrap();

        let tool = GlobTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "pattern": "**/*.py"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("No files matched"));
    }
}
