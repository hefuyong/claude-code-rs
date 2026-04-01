//! Code operation commands (git, review, PRs).

use crate::{CommandContext, Command, CommandOutput, CommandRegistry};
use std::process::Stdio;

/// Run a shell command and capture stdout+stderr.
async fn run_cmd(program: &str, args: &[&str], cwd: &std::path::Path) -> String {
    match tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                if stdout.is_empty() {
                    "(no output)".to_string()
                } else {
                    stdout.trim().to_string()
                }
            } else {
                format!("Error (exit {}):\n{}{}", output.status, stdout, stderr)
            }
        }
        Err(e) => format!("Failed to run '{program}': {e}"),
    }
}

/// Register code operation commands.
pub fn register_code_commands(registry: &mut CommandRegistry) {
    // /diff - Show current file changes
    registry.register(
        Command {
            name: "diff".into(),
            description: "Show current file changes (git diff)".into(),
            aliases: vec!["d".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                let output = if args.is_empty() {
                    run_cmd("git", &["diff"], &dir).await
                } else if args == "--staged" || args == "--cached" {
                    run_cmd("git", &["diff", "--cached"], &dir).await
                } else {
                    run_cmd("git", &["diff", "--", &args], &dir).await
                };
                Ok(CommandOutput::text(output))
            })
        }),
    );

    // /review - Review code changes
    registry.register(
        Command {
            name: "review".into(),
            description: "Review code changes in the working tree".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                // First check if we have staged changes; if so, review those,
                // otherwise review unstaged changes
                let staged_diff = run_cmd("git", &["diff", "--cached"], &dir).await;
                let has_staged = !staged_diff.contains("(no output)") && !staged_diff.starts_with("Error");

                let (diff_text, diff_label): (String, String) = if has_staged {
                    (staged_diff, "Staged changes".to_string())
                } else {
                    let unstaged = run_cmd("git", &["diff"], &dir).await;
                    if unstaged.contains("(no output)") || unstaged.starts_with("Error") {
                        // Try showing changes against a specific ref if provided
                        if !args.is_empty() {
                            let ref_diff = run_cmd("git", &["diff", &args], &dir).await;
                            (ref_diff, format!("Changes vs {args}"))
                        } else {
                            return Ok(CommandOutput::text(
                                "No changes found (neither staged nor unstaged).\n\
                                 Usage: /review [<ref>] to review changes against a specific ref."
                            ));
                        }
                    } else {
                        (unstaged, "Unstaged changes".to_string())
                    }
                };

                // Get stat summary
                let stat_args = if has_staged {
                    vec!["diff", "--cached", "--stat"]
                } else {
                    vec!["diff", "--stat"]
                };
                let stat = run_cmd("git", &stat_args.iter().map(|s| *s).collect::<Vec<_>>(), &dir).await;

                // Get the number of files changed
                let numstat_args = if has_staged {
                    vec!["diff", "--cached", "--numstat"]
                } else {
                    vec!["diff", "--numstat"]
                };
                let numstat = run_cmd("git", &numstat_args.iter().map(|s| *s).collect::<Vec<_>>(), &dir).await;
                let file_count = numstat.lines().count();
                let (additions, deletions) = numstat.lines().fold((0u64, 0u64), |(add, del), line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    let a: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let d: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    (add + a, del + d)
                });

                Ok(CommandOutput::text(format!(
                    "Code Review Summary\n\
                     ===================\n\n\
                     Reviewing: {diff_label}\n\
                     Files changed: {file_count}\n\
                     Additions:     +{additions}\n\
                     Deletions:     -{deletions}\n\n\
                     File summary:\n{stat}\n\n\
                     Full diff:\n```\n{diff_text}\n```\n\n\
                     Tip: ask Claude to review the diff above for detailed feedback."
                )))
            })
        }),
    );

    // /ultrareview - Deep code review
    registry.register(
        Command {
            name: "ultrareview".into(),
            description: "Deep code review with extended analysis".into(),
            aliases: vec!["ureview".into()],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let diff = run_cmd("git", &["diff", "--stat", "--diff-filter=ACMR"], &dir).await;
                Ok(CommandOutput::text(format!(
                    "Ultra Review (deep analysis)\n\
                     ============================\n\n\
                     Changed files:\n{diff}\n\n\
                     Analysis checklist:\n\
                     - [ ] Security vulnerabilities\n\
                     - [ ] Performance regressions\n\
                     - [ ] Error handling completeness\n\
                     - [ ] Test coverage gaps\n\
                     - [ ] API contract changes\n\n\
                     Tip: paste this output to Claude for a full deep review."
                )))
            })
        }),
    );

    // /commit - Create git commit
    registry.register(
        Command {
            name: "commit".into(),
            description: "Stage all changes and create a git commit".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let msg = args.trim().to_string();
            Box::pin(async move {
                if msg.is_empty() {
                    return Ok(CommandOutput::text(
                        "Usage: /commit <message>\n\
                         Stages all changes and creates a commit.",
                    ));
                }
                let add_out = run_cmd("git", &["add", "-A"], &dir).await;
                if add_out.contains("Error") {
                    return Ok(CommandOutput::text(format!("git add failed:\n{add_out}")));
                }
                let commit_out = run_cmd("git", &["commit", "-m", &msg], &dir).await;
                Ok(CommandOutput::text(commit_out))
            })
        }),
    );

    // /branch - Show/switch git branch
    registry.register(
        Command {
            name: "branch".into(),
            description: "Show current branch or switch branches".into(),
            aliases: vec!["br".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    let output = run_cmd("git", &["branch", "--show-current"], &dir).await;
                    Ok(CommandOutput::text(format!("Current branch: {output}")))
                } else {
                    let output = run_cmd("git", &["checkout", &args], &dir).await;
                    Ok(CommandOutput::text(output))
                }
            })
        }),
    );

    // /pr - Create pull request
    registry.register(
        Command {
            name: "pr".into(),
            description: "Create a pull request (requires gh CLI)".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                // Check if gh is installed
                let gh_check = run_cmd("gh", &["--version"], &dir).await;
                if gh_check.starts_with("Failed to run") {
                    return Ok(CommandOutput::text(
                        "Error: GitHub CLI (gh) is not installed.\n\
                         Install from: https://cli.github.com/\n\n\
                         Alternatively, create PRs manually:\n\
                         git push -u origin <branch>\n\
                         Then open the PR URL shown by git."
                    ));
                }

                if args.is_empty() {
                    // Show current PR status
                    let status = run_cmd("gh", &["pr", "status"], &dir).await;
                    Ok(CommandOutput::text(format!(
                        "PR Status\n\
                         =========\n\
                         {status}\n\n\
                         Usage: /pr <title> to create a new PR.\n\
                         Usage: /pr <title> | <body> to create with a body."
                    )))
                } else {
                    // Parse title and optional body separated by |
                    let (title, body) = if let Some(idx) = args.find('|') {
                        let t = args[..idx].trim().to_string();
                        let b = args[idx + 1..].trim().to_string();
                        (t, b)
                    } else {
                        (args.clone(), String::new())
                    };

                    let mut cmd_args = vec!["pr", "create", "--title", &title];
                    if !body.is_empty() {
                        cmd_args.push("--body");
                        cmd_args.push(&body);
                    } else {
                        cmd_args.push("--fill");
                    }

                    let output = run_cmd("gh", &cmd_args, &dir).await;
                    Ok(CommandOutput::text(format!("PR Creation\n===========\n{output}")))
                }
            })
        }),
    );

    // /pr_comments - Show PR comments
    registry.register(
        Command {
            name: "pr_comments".into(),
            description: "Show comments on the current pull request".into(),
            aliases: vec!["pr-comments".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                // Check if gh is installed
                let gh_check = run_cmd("gh", &["--version"], &dir).await;
                if gh_check.starts_with("Failed to run") {
                    return Ok(CommandOutput::text(
                        "Error: GitHub CLI (gh) is not installed.\n\
                         Install from: https://cli.github.com/"
                    ));
                }

                let pr_num = if args.is_empty() { String::new() } else { args };

                // Get PR comments with full details
                let mut view_args = vec!["pr", "view"];
                if !pr_num.is_empty() {
                    view_args.push(&pr_num);
                }
                view_args.extend_from_slice(&["--json", "number,title,comments,reviews"]);

                let output = run_cmd("gh", &view_args, &dir).await;

                if output.starts_with("Error") || output.starts_with("Failed") {
                    return Ok(CommandOutput::text(format!(
                        "Could not fetch PR comments.\n{output}\n\n\
                         Usage: /pr_comments [<number>]\n\
                         If no number is given, uses the PR for the current branch."
                    )));
                }

                // Parse the JSON response for a nice display
                match serde_json::from_str::<serde_json::Value>(&output) {
                    Ok(data) => {
                        let number = data.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
                        let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)");

                        let mut lines = vec![
                            format!("PR #{number}: {title}"),
                            "=".repeat(40),
                            String::new(),
                        ];

                        // Regular comments
                        if let Some(comments) = data.get("comments").and_then(|v| v.as_array()) {
                            if comments.is_empty() {
                                lines.push("No comments.".into());
                            } else {
                                lines.push(format!("Comments ({}):", comments.len()));
                                for comment in comments {
                                    let author = comment.get("author")
                                        .and_then(|a| a.get("login"))
                                        .and_then(|l| l.as_str())
                                        .unwrap_or("unknown");
                                    let body = comment.get("body")
                                        .and_then(|b| b.as_str())
                                        .unwrap_or("");
                                    let created = comment.get("createdAt")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("");
                                    lines.push(format!("\n  @{author} ({created}):"));
                                    for line in body.lines() {
                                        lines.push(format!("    {line}"));
                                    }
                                }
                            }
                        }

                        // Review comments
                        if let Some(reviews) = data.get("reviews").and_then(|v| v.as_array()) {
                            if !reviews.is_empty() {
                                lines.push(String::new());
                                lines.push(format!("Reviews ({}):", reviews.len()));
                                for review in reviews {
                                    let author = review.get("author")
                                        .and_then(|a| a.get("login"))
                                        .and_then(|l| l.as_str())
                                        .unwrap_or("unknown");
                                    let state = review.get("state")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("PENDING");
                                    let body = review.get("body")
                                        .and_then(|b| b.as_str())
                                        .unwrap_or("");
                                    lines.push(format!("\n  @{author} [{state}]:"));
                                    if !body.is_empty() {
                                        for line in body.lines() {
                                            lines.push(format!("    {line}"));
                                        }
                                    }
                                }
                            }
                        }

                        Ok(CommandOutput::text(lines.join("\n")))
                    }
                    Err(_) => {
                        // Fallback: show raw output
                        Ok(CommandOutput::text(format!("PR Comments:\n{output}")))
                    }
                }
            })
        }),
    );

    // /stash - Git stash operations
    registry.register(
        Command {
            name: "stash".into(),
            description: "Git stash operations".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() || args == "push" {
                    let output = run_cmd("git", &["stash", "push"], &dir).await;
                    Ok(CommandOutput::text(output))
                } else if args == "pop" {
                    let output = run_cmd("git", &["stash", "pop"], &dir).await;
                    Ok(CommandOutput::text(output))
                } else if args == "list" {
                    let output = run_cmd("git", &["stash", "list"], &dir).await;
                    Ok(CommandOutput::text(output))
                } else {
                    Ok(CommandOutput::text("Usage: /stash [push|pop|list]"))
                }
            })
        }),
    );

    // /blame - Show git blame for a file
    registry.register(
        Command {
            name: "blame".into(),
            description: "Show git blame for a file".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text("Usage: /blame <file>"))
                } else {
                    let output = run_cmd("git", &["blame", &args], &dir).await;
                    Ok(CommandOutput::text(output))
                }
            })
        }),
    );

    // /autofix-pr - Auto-fix PR issues
    registry.register(
        Command {
            name: "autofix-pr".into(),
            description: "Automatically fix issues flagged in PR review".into(),
            aliases: vec!["autofix".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                // Check if gh is installed
                let gh_check = run_cmd("gh", &["--version"], &dir).await;
                if gh_check.starts_with("Failed to run") {
                    return Ok(CommandOutput::text(
                        "Error: GitHub CLI (gh) is not installed.\n\
                         Install from: https://cli.github.com/"
                    ));
                }

                let pr_num = if args.is_empty() { String::new() } else { args };

                // Get PR check status
                let mut checks_args = vec!["pr", "checks"];
                if !pr_num.is_empty() {
                    checks_args.push(&pr_num);
                }
                let checks_output = run_cmd("gh", &checks_args, &dir).await;

                // Get review decision
                let mut review_args = vec!["pr", "view"];
                if !pr_num.is_empty() {
                    review_args.push(&pr_num);
                }
                review_args.extend_from_slice(&["--json", "reviewDecision,statusCheckRollup"]);
                let review_output = run_cmd("gh", &review_args, &dir).await;

                // Parse check results to find failures
                let mut failing_checks = Vec::new();
                let mut passing_checks = Vec::new();
                for line in checks_output.lines() {
                    let line_lower = line.to_lowercase();
                    if line_lower.contains("fail") || line_lower.contains("error") {
                        failing_checks.push(line.trim().to_string());
                    } else if line_lower.contains("pass") || line_lower.contains("success") {
                        passing_checks.push(line.trim().to_string());
                    }
                }

                let mut result_lines = vec![
                    "Auto-fix PR Analysis".to_string(),
                    "====================".to_string(),
                    String::new(),
                ];

                // Show review decision if available
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&review_output) {
                    if let Some(decision) = data.get("reviewDecision").and_then(|v| v.as_str()) {
                        result_lines.push(format!("Review decision: {decision}"));
                    }
                }

                result_lines.push(String::new());

                if failing_checks.is_empty() && !checks_output.contains("Error") {
                    result_lines.push("All checks passing.".into());
                    if !passing_checks.is_empty() {
                        result_lines.push(format!("({} check(s) green)", passing_checks.len()));
                    }
                } else if checks_output.starts_with("Error") || checks_output.starts_with("Failed") {
                    result_lines.push(format!("Could not fetch checks: {checks_output}"));
                } else {
                    result_lines.push(format!("Failing checks ({}):", failing_checks.len()));
                    for check in &failing_checks {
                        result_lines.push(format!("  - {check}"));
                    }
                    if !passing_checks.is_empty() {
                        result_lines.push(format!(
                            "\nPassing checks: {} of {}",
                            passing_checks.len(),
                            passing_checks.len() + failing_checks.len()
                        ));
                    }
                    result_lines.push(String::new());
                    result_lines.push(
                        "To auto-fix: ask Claude to read the failing check logs and apply fixes.".into()
                    );
                    result_lines.push(
                        "Example: \"Read the CI failures above and fix all issues.\"".into()
                    );
                }

                Ok(CommandOutput::text(result_lines.join("\n")))
            })
        }),
    );
}
