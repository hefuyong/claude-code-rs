//! Core tool abstractions for Claude Code RS.
//!
//! Defines the [`Tool`] trait that every tool must implement, the
//! [`ToolRegistry`] that holds available tools, and the [`ToolExecutor`]
//! that runs tool calls with concurrency control.

use async_trait::async_trait;
use cc_error::CcError;
use cc_permissions::PermissionContext;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// ── Tool trait ───────────────────────────────────────────────────────

/// The core trait every tool must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool (e.g. "bash", "file_read").
    fn name(&self) -> &str;

    /// A human-readable description shown to the model.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's expected input.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given input.
    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError>;

    /// Whether this tool only reads state and never mutates anything.
    fn is_read_only(&self) -> bool {
        false
    }

    /// Whether multiple instances of this tool can safely run in parallel.
    fn is_concurrency_safe(&self) -> bool {
        false
    }
}

// ── Supporting types ─────────────────────────────────────────────────

/// Runtime context passed to every tool invocation.
pub struct ToolContext {
    /// The working directory for relative path resolution.
    pub working_directory: PathBuf,
    /// The permission context governing what is allowed.
    pub permission_context: PermissionContext,
}

/// The output returned by a tool after execution.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// The textual content of the result.
    pub content: String,
    /// Whether this output represents an error.
    pub is_error: bool,
}

impl ToolOutput {
    /// Create a successful output.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    /// Create an error output.
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// A pending tool call, typically received from the model.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// The unique ID for this call (matches the API `tool_use.id`).
    pub id: String,
    /// Tool name to invoke.
    pub name: String,
    /// JSON input for the tool.
    pub input: serde_json::Value,
}

// ── ToolRegistry ─────────────────────────────────────────────────────

/// Registry that maps tool names to their implementations.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Panics if a tool with the same name already exists.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            panic!("Duplicate tool registration: {}", name);
        }
        self.tools.insert(name, tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// Return a sorted list of registered tool names.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Convert every registered tool into its API-facing [`ToolDefinition`].
    pub fn to_api_tools(&self) -> Vec<cc_types::ToolDefinition> {
        let mut defs: Vec<cc_types::ToolDefinition> = self
            .tools
            .values()
            .map(|t| cc_types::ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── ToolExecutor ─────────────────────────────────────────────────────

/// Executes tool calls against a shared [`ToolRegistry`], handling
/// concurrency: tools that declare `is_concurrency_safe() == true` can
/// run in parallel, while the rest are executed sequentially.
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    /// Wrap a registry in an executor.
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Get a reference to the underlying tool registry.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Execute a single tool call.
    pub async fn execute(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| CcError::NotFound(format!("Unknown tool: {}", tool_name)))?;

        // Permission check.
        let result = ctx.permission_context.check(tool_name, &input);
        match result {
            cc_permissions::PermissionCheckResult::Allow => {}
            cc_permissions::PermissionCheckResult::Deny { reason } => {
                return Err(CcError::PermissionDenied(reason));
            }
            cc_permissions::PermissionCheckResult::Ask { message } => {
                // In a real implementation this would pause and ask the user.
                // For now we deny with an explanation.
                return Err(CcError::PermissionDenied(format!(
                    "Permission required: {}",
                    message
                )));
            }
        }

        tracing::debug!(tool = tool_name, "executing tool");
        tool.call(input, ctx).await
    }

    /// Execute a batch of tool calls.
    ///
    /// Concurrency-safe calls are dispatched in parallel via a
    /// [`tokio::task::JoinSet`]. Non-safe calls run sequentially
    /// after all parallel ones have finished.
    pub async fn execute_batch(
        &self,
        calls: Vec<ToolCall>,
        ctx: &ToolContext,
    ) -> Vec<(String, Result<ToolOutput, CcError>)> {
        // Partition calls.
        let mut parallel_calls = Vec::new();
        let mut sequential_calls = Vec::new();

        for call in calls {
            let is_safe = self
                .registry
                .get(&call.name)
                .map(|t| t.is_concurrency_safe())
                .unwrap_or(false);
            if is_safe {
                parallel_calls.push(call);
            } else {
                sequential_calls.push(call);
            }
        }

        let mut results: Vec<(String, Result<ToolOutput, CcError>)> = Vec::new();

        // Run parallel calls.
        if !parallel_calls.is_empty() {
            let mut join_set = tokio::task::JoinSet::new();

            for call in parallel_calls {
                let registry = Arc::clone(&self.registry);
                let call_id = call.id.clone();
                let call_name = call.name.clone();
                let call_input = call.input.clone();

                // We need to build a per-task context because ToolContext
                // is not Send (it holds PermissionContext which is Clone).
                let task_ctx = ToolContext {
                    working_directory: ctx.working_directory.clone(),
                    permission_context: ctx.permission_context.clone(),
                };

                join_set.spawn(async move {
                    let result = match registry.get(&call_name) {
                        Some(tool) => {
                            let perm = task_ctx.permission_context.check(&call_name, &call_input);
                            match perm {
                                cc_permissions::PermissionCheckResult::Allow => {
                                    tool.call(call_input, &task_ctx).await
                                }
                                cc_permissions::PermissionCheckResult::Deny { reason } => {
                                    Err(CcError::PermissionDenied(reason))
                                }
                                cc_permissions::PermissionCheckResult::Ask { message } => {
                                    Err(CcError::PermissionDenied(format!(
                                        "Permission required: {}",
                                        message
                                    )))
                                }
                            }
                        }
                        None => Err(CcError::NotFound(format!("Unknown tool: {}", call_name))),
                    };
                    (call_id, result)
                });
            }

            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok(pair) => results.push(pair),
                    Err(join_err) => {
                        results.push((
                            String::new(),
                            Err(CcError::Internal(format!("Task join error: {}", join_err))),
                        ));
                    }
                }
            }
        }

        // Run sequential calls.
        for call in sequential_calls {
            let result = self.execute(&call.name, call.input, ctx).await;
            results.push((call.id, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_permissions::PermissionMode;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input back"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                }
            })
        }
        async fn call(
            &self,
            input: serde_json::Value,
            _ctx: &ToolContext,
        ) -> Result<ToolOutput, CcError> {
            let text = input
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("(empty)");
            Ok(ToolOutput::success(text))
        }
        fn is_read_only(&self) -> bool {
            true
        }
        fn is_concurrency_safe(&self) -> bool {
            true
        }
    }

    #[test]
    fn registry_basics() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(EchoTool));
        assert!(reg.get("echo").is_some());
        assert!(reg.get("missing").is_none());
        assert_eq!(reg.list(), vec!["echo"]);
    }

    #[tokio::test]
    async fn executor_runs_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(EchoTool));
        let executor = ToolExecutor::new(Arc::new(reg));

        let ctx = ToolContext {
            working_directory: PathBuf::from("."),
            permission_context: PermissionContext::new(PermissionMode::Bypass, vec![]),
        };

        let out = executor
            .execute("echo", serde_json::json!({"text": "hello"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out.content, "hello");
        assert!(!out.is_error);
    }

    #[tokio::test]
    async fn executor_batch() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(EchoTool));
        let executor = ToolExecutor::new(Arc::new(reg));

        let ctx = ToolContext {
            working_directory: PathBuf::from("."),
            permission_context: PermissionContext::new(PermissionMode::Bypass, vec![]),
        };

        let calls = vec![
            ToolCall {
                id: "1".into(),
                name: "echo".into(),
                input: serde_json::json!({"text": "a"}),
            },
            ToolCall {
                id: "2".into(),
                name: "echo".into(),
                input: serde_json::json!({"text": "b"}),
            },
        ];

        let results = executor.execute_batch(calls, &ctx).await;
        assert_eq!(results.len(), 2);
        for (_id, res) in &results {
            assert!(res.is_ok());
        }
    }
}
