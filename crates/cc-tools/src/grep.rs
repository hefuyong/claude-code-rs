//! GrepTool -- search file contents with regex.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use regex::Regex;
use std::path::{Path, PathBuf};

pub struct GrepTool;

const MAX_RESULTS: usize = 500;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using a regular expression"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search (defaults to working directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.rs\")"
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
        let pattern_str = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("grep", "Missing required field: pattern"))?;

        let search_path = match input.get("path").and_then(|v| v.as_str()) {
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

        let include = input
            .get("include")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let re = Regex::new(pattern_str)
            .map_err(|e| CcError::tool("grep", format!("Invalid regex: {}", e)))?;

        let include_glob = match &include {
            Some(pat) => Some(
                globset::GlobBuilder::new(pat)
                    .literal_separator(false)
                    .build()
                    .map_err(|e| CcError::tool("grep", format!("Invalid include pattern: {}", e)))?
                    .compile_matcher(),
            ),
            None => None,
        };

        // Collect files to search.
        let files = tokio::task::spawn_blocking({
            let search_path = search_path.clone();
            let include_glob = include_glob.clone();
            move || {
                let mut files = Vec::new();
                if search_path.is_file() {
                    files.push(search_path);
                } else {
                    collect_files(&search_path, &include_glob, &mut files);
                }
                files.sort();
                files
            }
        })
        .await
        .map_err(|e| CcError::Internal(format!("Join error: {}", e)))?;

        // Search files.
        let mut matches = Vec::new();
        let mut truncated = false;

        for file in &files {
            let content = match tokio::fs::read_to_string(file).await {
                Ok(c) => c,
                Err(_) => continue, // skip binary / unreadable files
            };

            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(format!(
                        "{}:{}:{}",
                        file.display(),
                        line_num + 1,
                        line
                    ));
                    if matches.len() >= MAX_RESULTS {
                        truncated = true;
                        break;
                    }
                }
            }

            if truncated {
                break;
            }
        }

        if matches.is_empty() {
            return Ok(ToolOutput::success("No matches found."));
        }

        let mut output = matches.join("\n");
        if truncated {
            output.push_str(&format!("\n\n(results truncated at {} matches)", MAX_RESULTS));
        } else {
            output.push_str(&format!("\n\n({} matches)", matches.len()));
        }

        Ok(ToolOutput::success(output))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}

/// Recursively collect files, filtering by the include glob if provided.
fn collect_files(
    dir: &Path,
    include: &Option<globset::GlobMatcher>,
    files: &mut Vec<PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            collect_files(&path, include, files);
        } else {
            if let Some(glob) = include {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !glob.is_match(name.as_ref()) {
                    continue;
                }
            }
            files.push(path);
        }
    }
}
