//! Diagnostic and debugging commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};
use std::process::Stdio;

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
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map(|o| {
                        if o.status.success() {
                            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            Some(ver)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                checks.push(format!(
                    "[{}] git installed{}",
                    if git_ok.is_some() { "OK" } else { "FAIL" },
                    git_ok.as_ref().map(|v| format!(" ({v})")).unwrap_or_default(),
                ));

                // Check if we're in a git repo
                let repo_ok = tokio::process::Command::new("git")
                    .args(["rev-parse", "--git-dir"])
                    .current_dir(&dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] git repository",
                    if repo_ok { "OK" } else { "WARN" }
                ));

                // Check API key
                let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
                let api_key_valid = api_key.as_ref().map(|k| k.starts_with("sk-ant-")).unwrap_or(false);
                let api_key_set = api_key.is_some();
                checks.push(format!(
                    "[{}] ANTHROPIC_API_KEY set{}",
                    if api_key_set { "OK" } else { "FAIL" },
                    if api_key_set && !api_key_valid { " (unusual format)" } else { "" },
                ));

                // Check gh CLI
                let gh_ver = tokio::process::Command::new("gh")
                    .args(["--version"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map(|o| {
                        if o.status.success() {
                            let ver = String::from_utf8_lossy(&o.stdout)
                                .lines()
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            Some(ver)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                checks.push(format!(
                    "[{}] gh CLI installed{}",
                    if gh_ver.is_some() { "OK" } else { "WARN" },
                    gh_ver.as_ref().map(|v| format!(" ({v})")).unwrap_or_default(),
                ));

                // Check node (for MCP servers)
                let node_ver = tokio::process::Command::new("node")
                    .args(["--version"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                checks.push(format!(
                    "[{}] node.js installed (for MCP servers){}",
                    if node_ver.is_some() { "OK" } else { "WARN" },
                    node_ver.as_ref().map(|v| format!(" ({v})")).unwrap_or_default(),
                ));

                // Check config dir
                let claude_dir = dirs::home_dir().map(|h| h.join(".claude"));
                let config_dir_exists = claude_dir.as_ref().map(|p| p.exists()).unwrap_or(false);
                checks.push(format!(
                    "[{}] ~/.claude config directory",
                    if config_dir_exists { "OK" } else { "WARN" }
                ));

                // Check sessions dir writable
                let sessions_dir = claude_dir.as_ref().map(|p| p.join("sessions"));
                let sessions_writable = if let Some(ref sdir) = sessions_dir {
                    if sdir.exists() {
                        // Try writing a temp file
                        let test_path = sdir.join(".doctor-test");
                        let result = tokio::fs::write(&test_path, "test").await.is_ok();
                        let _ = tokio::fs::remove_file(&test_path).await;
                        result
                    } else {
                        // Try creating it
                        match tokio::fs::create_dir_all(sdir).await {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    }
                } else {
                    false
                };
                checks.push(format!(
                    "[{}] sessions directory writable",
                    if sessions_writable { "OK" } else { "FAIL" }
                ));

                // Check settings.json
                let settings_exists = claude_dir
                    .as_ref()
                    .map(|p| p.join("settings.json").exists())
                    .unwrap_or(false);
                checks.push(format!(
                    "[{}] settings.json",
                    if settings_exists { "OK" } else { "WARN" }
                ));

                // Summary
                let fail_count = checks.iter().filter(|c| c.starts_with("[FAIL]")).count();
                let warn_count = checks.iter().filter(|c| c.starts_with("[WARN]")).count();
                let ok_count = checks.iter().filter(|c| c.starts_with("[OK]")).count();

                let status = if fail_count > 0 {
                    "Issues found"
                } else if warn_count > 0 {
                    "OK (with warnings)"
                } else {
                    "All checks passed"
                };

                Ok(CommandOutput::text(format!(
                    "System Diagnostics\n\
                     ==================\n\
                     {}\n\n\
                     Summary: {status} ({ok_count} ok, {warn_count} warn, {fail_count} fail)\n\
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
                        "No log directory found at: {}\n\
                         Logs will be created when debug mode is enabled.",
                        log_dir.display()
                    )));
                }

                let limit: usize = args.parse().unwrap_or(50);

                // Try to find latest.log or the most recent log file
                let latest_log = log_dir.join("latest.log");
                let log_file = if latest_log.exists() {
                    latest_log
                } else {
                    // Find the most recent .log file
                    let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
                    if let Ok(mut read_dir) = tokio::fs::read_dir(&log_dir).await {
                        while let Ok(Some(entry)) = read_dir.next_entry().await {
                            let path = entry.path();
                            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                                if let Ok(meta) = entry.metadata().await {
                                    if let Ok(modified) = meta.modified() {
                                        if newest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                                            newest = Some((path, modified));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    match newest {
                        Some((path, _)) => path,
                        None => {
                            return Ok(CommandOutput::text(format!(
                                "No log files found in: {}",
                                log_dir.display()
                            )));
                        }
                    }
                };

                match tokio::fs::read_to_string(&log_file).await {
                    Ok(content) => {
                        let all_lines: Vec<&str> = content.lines().collect();
                        let total = all_lines.len();
                        let start = total.saturating_sub(limit);
                        let shown_lines = &all_lines[start..];

                        let file_size = tokio::fs::metadata(&log_file).await
                            .map(|m| m.len())
                            .unwrap_or(0);

                        let mut output = format!(
                            "Log: {} ({} bytes, {} total lines)\n\
                             Showing last {} lines:\n\
                             {}\n",
                            log_file.display(),
                            file_size,
                            total,
                            shown_lines.len(),
                            "=".repeat(50),
                        );

                        for line in shown_lines {
                            output.push_str(line);
                            output.push('\n');
                        }

                        Ok(CommandOutput::text(output))
                    }
                    Err(e) => Ok(CommandOutput::text(format!(
                        "Error reading log file {}: {e}",
                        log_file.display()
                    ))),
                }
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
                let est_total = est_input + est_output;

                // Estimate cache tokens (roughly 30% of input on multi-turn)
                let est_cache_read = if turns > 1 { est_input * 30 / 100 } else { 0 };
                let est_cache_write = if turns > 0 { est_input * 10 / 100 } else { 0 };

                // Per-model pricing (per 1M tokens)
                let (input_price, output_price, cache_read_price, cache_write_price) =
                    if model.contains("opus") {
                        (15.0, 75.0, 1.5, 18.75)
                    } else if model.contains("sonnet") {
                        (3.0, 15.0, 0.3, 3.75)
                    } else if model.contains("haiku") {
                        (0.25, 1.25, 0.03, 0.3)
                    } else {
                        (3.0, 15.0, 0.3, 3.75) // default to sonnet pricing
                    };

                let input_cost = est_input as f64 * input_price / 1_000_000.0;
                let output_cost = est_output as f64 * output_price / 1_000_000.0;
                let cache_read_cost = est_cache_read as f64 * cache_read_price / 1_000_000.0;
                let cache_write_cost = est_cache_write as f64 * cache_write_price / 1_000_000.0;
                let est_total_cost = input_cost + output_cost + cache_read_cost + cache_write_cost;

                Ok(CommandOutput::text(format!(
                    "Token Usage\n\
                     ===========\n\
                     Model:           {model}\n\
                     Total turns:     {turns}\n\n\
                     Token Breakdown\n\
                     ---------------\n\
                     Input tokens:    ~{est_input}\n\
                     Output tokens:   ~{est_output}\n\
                     Cache read:      ~{est_cache_read}\n\
                     Cache write:     ~{est_cache_write}\n\
                     Total tokens:    ~{est_total}\n\n\
                     Cost Breakdown (estimated)\n\
                     --------------------------\n\
                     Input:           ${input_cost:.4} (${input_price}/MTok)\n\
                     Output:          ${output_cost:.4} (${output_price}/MTok)\n\
                     Cache read:      ${cache_read_cost:.4} (${cache_read_price}/MTok)\n\
                     Cache write:     ${cache_write_cost:.4} (${cache_write_price}/MTok)\n\
                     Estimated total: ${est_total_cost:.4}\n\
                     Reported total:  {cost}\n\n\
                     Note: actual usage may differ; estimates based on average tokens per turn.",
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
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let mut results = Vec::new();

                // 1. CPU benchmark
                let start = std::time::Instant::now();
                let mut sum: u64 = 0;
                for i in 0..1_000_000u64 {
                    sum = sum.wrapping_add(i);
                }
                let cpu_ms = start.elapsed().as_millis();
                let _ = sum; // prevent optimizer removal
                results.push(format!("CPU (1M iterations):   {cpu_ms}ms"));

                // 2. Filesystem benchmark
                let fs_start = std::time::Instant::now();
                let tmp = std::env::temp_dir().join("claude-code-bench.tmp");
                let data = "benchmark test data\n".repeat(1000);
                let _ = tokio::fs::write(&tmp, &data).await;
                let _ = tokio::fs::read(&tmp).await;
                let _ = tokio::fs::remove_file(&tmp).await;
                let fs_ms = fs_start.elapsed().as_millis();
                results.push(format!("Filesystem (R/W 20KB): {fs_ms}ms"));

                // 3. Git status benchmark
                let git_start = std::time::Instant::now();
                let git_result = tokio::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await;
                let git_ms = git_start.elapsed().as_millis();
                let git_label = if git_result.map(|o| o.status.success()).unwrap_or(false) {
                    format!("git status:            {git_ms}ms")
                } else {
                    format!("git status:            {git_ms}ms (not a repo)")
                };
                results.push(git_label);

                // 4. Directory listing benchmark
                let dir_start = std::time::Instant::now();
                let mut file_count = 0u64;
                if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
                    while let Ok(Some(_)) = rd.next_entry().await {
                        file_count += 1;
                    }
                }
                let dir_ms = dir_start.elapsed().as_millis();
                results.push(format!("Dir listing ({file_count} entries): {dir_ms}ms"));

                // 5. API ping (if key set)
                let api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
                if api_key {
                    results.push("API ping:              (skipped - use /health)".into());
                } else {
                    results.push("API ping:              (no API key set)".into());
                }

                let total_ms = cpu_ms + fs_ms + git_ms + dir_ms;

                Ok(CommandOutput::text(format!(
                    "Performance Benchmark\n\
                     =====================\n\
                     {}\n\n\
                     Total:                 {total_ms}ms",
                    results.join("\n"),
                )))
            })
        }),
    );
}
