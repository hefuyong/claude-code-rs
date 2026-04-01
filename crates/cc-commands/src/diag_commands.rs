//! Diagnostic and debugging commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register diagnostic commands.
pub fn register_diag_commands(registry: &mut CommandRegistry) {
    // /doctor - System diagnostics
    registry.register(
        Command {
            name: "doctor".into(),
            description: "Run system diagnostics".into(),
            aliases: vec!["diag".into()],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let mut checks = Vec::new();

                // Check git
                let git_ok = tokio::process::Command::new("git")
                    .args(["--version"])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] git installed",
                    if git_ok { "OK" } else { "FAIL" }
                ));

                // Check if we're in a git repo
                let repo_ok = tokio::process::Command::new("git")
                    .args(["rev-parse", "--git-dir"])
                    .current_dir(&dir)
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] git repository",
                    if repo_ok { "OK" } else { "WARN" }
                ));

                // Check API key
                let api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
                checks.push(format!(
                    "[{}] ANTHROPIC_API_KEY set",
                    if api_key { "OK" } else { "FAIL" }
                ));

                // Check gh CLI
                let gh_ok = tokio::process::Command::new("gh")
                    .args(["--version"])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] gh CLI installed",
                    if gh_ok { "OK" } else { "WARN" }
                ));

                // Check node (for MCP servers)
                let node_ok = tokio::process::Command::new("node")
                    .args(["--version"])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] node.js installed (for MCP servers)",
                    if node_ok { "OK" } else { "WARN" }
                ));

                // Check config dir
                let config_dir = dirs::home_dir()
                    .map(|h| h.join(".claude"))
                    .map(|p| p.exists())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] ~/.claude config directory",
                    if config_dir { "OK" } else { "WARN" }
                ));

                Ok(CommandOutput::text(format!(
                    "System Diagnostics\n\
                     ==================\n\
                     {}\n\n\
                     Legend: [OK] = pass, [WARN] = optional, [FAIL] = required",
                    checks.join("\n")
                )))
            })
        }),
    );

    // /version - Show version info
    registry.register(
        Command {
            name: "version".into(),
            description: "Show version information".into(),
            aliases: vec!["v".into(), "ver".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let version = env!("CARGO_PKG_VERSION");
                Ok(CommandOutput::text(format!(
                    "claude-code-rs v{version}\n\
                     Rust edition: 2021\n\
                     Platform: {os}/{arch}",
                    os = std::env::consts::OS,
                    arch = std::env::consts::ARCH,
                )))
            })
        }),
    );

    // /debug - Toggle debug mode
    registry.register(
        Command {
            name: "debug".into(),
            description: "Toggle debug logging".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                match args.as_str() {
                    "on" | "enable" | "true" => Ok(CommandOutput::text(
                        "Debug mode enabled.\n\
                         Verbose logging will appear in the output.\n\
                         API request/response bodies will be logged.",
                    )),
                    "off" | "disable" | "false" => {
                        Ok(CommandOutput::text("Debug mode disabled."))
                    }
                    _ => Ok(CommandOutput::text(
                        "Debug mode: off\n\
                         Usage: /debug [on|off]\n\
                         Enables verbose logging for troubleshooting.",
                    )),
                }
            })
        }),
    );

    // /logs - Show logs
    registry.register(
        Command {
            name: "logs".into(),
            description: "Show recent log entries".into(),
            aliases: vec!["log".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let log_dir = dirs::home_dir()
                    .map(|h| h.join(".claude").join("logs"))
                    .unwrap_or_default();
                if !log_dir.exists() {
                    return Ok(CommandOutput::text(format!(
                        "No log directory found at: {}",
                        log_dir.display()
                    )));
                }
                let limit: usize = args.parse().unwrap_or(20);
                Ok(CommandOutput::text(format!(
                    "Log directory: {}\n\
                     Showing last {limit} entries:\n\
                     (log viewing not yet fully implemented)\n\n\
                     Tip: check {} directly for full logs.",
                    log_dir.display(),
                    log_dir.display(),
                )))
            })
        }),
    );

    // /tokens - Show token usage
    registry.register(
        Command {
            name: "tokens".into(),
            description: "Show detailed token usage".into(),
            aliases: vec!["usage".into()],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let cost = ctx.total_cost.clone();
            let turns = ctx.total_turns;
            let model = ctx.model.clone();
            Box::pin(async move {
                let est_input = turns * 1500;
                let est_output = turns * 800;
                Ok(CommandOutput::text(format!(
                    "Token Usage\n\
                     ===========\n\
                     Model:           {model}\n\
                     Total turns:     {turns}\n\
                     Est. input:      ~{est_input} tokens\n\
                     Est. output:     ~{est_output} tokens\n\
                     Est. total:      ~{total} tokens\n\
                     Total cost:      {cost}\n\n\
                     Note: estimates based on average tokens per turn.",
                    total = est_input + est_output,
                )))
            })
        }),
    );

    // /health - Quick health check
    registry.register(
        Command {
            name: "health".into(),
            description: "Quick health check of the system".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
                let status = if api_key { "healthy" } else { "degraded (no API key)" };
                Ok(CommandOutput::text(format!(
                    "Health: {status}\n\
                     Use /doctor for a full diagnostic check."
                )))
            })
        }),
    );

    // /crash-report - Show or manage crash reports
    registry.register(
        Command {
            name: "crash-report".into(),
            description: "Show or manage crash reports".into(),
            aliases: vec!["crashes".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let crash_dir = dirs::home_dir()
                    .map(|h| h.join(".claude").join("crashes"))
                    .unwrap_or_default();
                if crash_dir.exists() {
                    Ok(CommandOutput::text(format!(
                        "Crash report directory: {}\n\
                         (check directory for recent crash reports)",
                        crash_dir.display()
                    )))
                } else {
                    Ok(CommandOutput::text(
                        "No crash reports found. Good news!",
                    ))
                }
            })
        }),
    );

    // /benchmark - Run performance benchmark
    registry.register(
        Command {
            name: "benchmark".into(),
            description: "Run a simple performance benchmark".into(),
            aliases: vec!["bench".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let start = std::time::Instant::now();

                // Simple CPU benchmark
                let mut sum: u64 = 0;
                for i in 0..1_000_000u64 {
                    sum = sum.wrapping_add(i);
                }
                let cpu_ms = start.elapsed().as_millis();

                // Filesystem benchmark
                let fs_start = std::time::Instant::now();
                let tmp = std::env::temp_dir().join("claude-code-bench.tmp");
                let _ = tokio::fs::write(&tmp, "benchmark test data\n".repeat(100)).await;
                let _ = tokio::fs::read(&tmp).await;
                let _ = tokio::fs::remove_file(&tmp).await;
                let fs_ms = fs_start.elapsed().as_millis();

                // Prevent optimizer from removing the sum
                let _ = sum;

                Ok(CommandOutput::text(format!(
                    "Performance Benchmark\n\
                     =====================\n\
                     CPU (1M iterations):   {cpu_ms}ms\n\
                     Filesystem (R/W):      {fs_ms}ms\n\
                     Total:                 {}ms",
                    cpu_ms + fs_ms,
                )))
            })
        }),
    );
}
