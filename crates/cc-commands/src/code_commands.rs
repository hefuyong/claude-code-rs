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
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let diff = run_cmd("git", &["diff", "--stat"], &dir).await;
                let staged = run_cmd("git", &["diff", "--cached", "--stat"], &dir).await;
                Ok(CommandOutput::text(format!(
                    "Code Review Summary\n\
                     ===================\n\n\
                     Unstaged changes:\n{diff}\n\n\
                     Staged changes:\n{staged}\n\n\
                     Tip: ask Claude to review specific files for detailed feedback."
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
                if args.is_empty() {
                    let output = run_cmd("gh", &["pr", "view", "--web"], &dir).await;
                    Ok(CommandOutput::text(format!(
                        "Current PR status:\n{output}\n\n\
                         Usage: /pr <title> to create a new PR."
                    )))
                } else {
                    let output =
                        run_cmd("gh", &["pr", "create", "--title", &args, "--fill"], &dir).await;
                    Ok(CommandOutput::text(output))
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
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let output = run_cmd(
                    "gh",
                    &["pr", "view", "--json", "comments", "--jq", ".comments[].body"],
                    &dir,
                )
                .await;
                Ok(CommandOutput::text(format!("PR Comments:\n{output}")))
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
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let comments = run_cmd(
                    "gh",
                    &["pr", "view", "--json", "reviewDecision"],
                    &dir,
                )
                .await;
                Ok(CommandOutput::text(format!(
                    "Auto-fix PR\n\
                     ===========\n\
                     PR review status: {comments}\n\n\
                     To auto-fix: ask Claude to read the PR comments and apply fixes.\n\
                     Example: \"Read the PR review comments and fix all issues.\""
                )))
            })
        }),
    );
}
