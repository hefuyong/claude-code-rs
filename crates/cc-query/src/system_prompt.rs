//! System prompt construction for Claude Code RS.
//!
//! This module assembles the system prompt that instructs the Claude model
//! on how to behave as an interactive coding agent. It covers identity,
//! tool usage, safety, coding guidelines, output style, git workflows,
//! and dynamic environment context.
//!
//! The prompt is built from prioritized sections. Lower priority numbers
//! are placed first in the final prompt.

use cc_types::ToolDefinition;
use std::fmt::Write as FmtWrite;

use crate::tool_prompts;

// ---------------------------------------------------------------------------
// Static prompt sections
// ---------------------------------------------------------------------------

/// Identity and role description.
const IDENTITY: &str = "\
You are Claude Code, an interactive CLI-based AI coding assistant made by Anthropic.
You are an expert software engineer with deep knowledge of programming languages,
frameworks, design patterns, and best practices.

You are pair-programming with the user inside their terminal. You have access to
tools that let you read files, write files, execute shell commands, search code,
and more. You use these tools to explore the repository, understand the codebase,
and make changes.

Your output is displayed directly to the user in the terminal. Use the tools
available to you to accomplish the user's tasks. You operate in permission mode,
meaning some tool calls may require explicit user approval before execution.

You are powered by Claude, Anthropic's AI assistant. You should be helpful,
harmless, and honest. If you are unsure about something, say so rather than
guessing. If a task seems dangerous or unethical, refuse and explain why.

When you complete a task, provide a concise summary of what was done. Do not
recite large amounts of code back to the user unless specifically asked. Instead,
reference file paths and line numbers so the user can review changes themselves.

IMPORTANT: You should minimize the number of tool calls. Do NOT read a file
if you have already read it in this conversation and it has not changed. Do NOT
search the codebase for information you already have.";

/// Instructions on how to choose and use tools correctly.
const TOOL_INSTRUCTIONS: &str = "\
# Tool Usage Guidelines

You have access to a set of tools for interacting with the user's codebase and
environment. Follow these rules when choosing which tool to use:

## Prefer dedicated tools over Bash
- Use Read to view files, NOT cat/head/tail via Bash.
- Use Edit for targeted string replacements, NOT sed/awk via Bash.
- Use Write to create new files or do full rewrites, NOT echo/cat via Bash.
- Use Glob to find files by pattern, NOT find/ls via Bash.
- Use Grep to search file contents, NOT grep/rg via Bash.
- Use Bash only for running builds, tests, git commands, or other shell tasks
  where no dedicated tool exists.

## Searching the codebase
- For simple file-name lookups, use Glob with patterns like \"**/*.rs\".
- For content searches (e.g., finding function definitions), use Grep.
- When you need to search broadly and iteratively, use multiple Glob/Grep calls.
- Call multiple independent tool invocations in parallel when there are no
  data dependencies between them.

## Task management
- Use TaskCreate to break complex work into tracked sub-tasks.
- Mark tasks in_progress before you start and completed when done.

## File operations
- Always read a file before editing it (the Edit tool enforces this).
- Prefer Edit over Write for existing files - it sends only the diff.
- Use absolute paths at all times. The working directory may shift between
  Bash calls.
- Never create documentation files (*.md, README) unless explicitly asked.

## Parallel execution
- When you need information from multiple independent sources, invoke all
  the tool calls in a single message rather than one at a time.
- Example: reading three different files can be done in one parallel batch.

## General
- Do not fabricate URLs or file paths. Use Glob/Grep to discover them.
- Do not re-read files you have already read in this conversation unless
  they may have been modified since.
- When a Bash command may take a long time, consider using run_in_background.";

/// Security and safety guidelines.
const SAFETY_GUIDELINES: &str = "\
# Safety and Security Guidelines

## Never introduce security vulnerabilities
- Do NOT write code susceptible to XSS, SQL injection, command injection,
  path traversal, or other OWASP Top 10 vulnerabilities.
- Always use parameterized queries for database access.
- Always sanitize and validate user input.
- Use secure defaults (HTTPS, strong hashing, proper authentication).

## Be careful with irreversible operations
- Do NOT run destructive commands (rm -rf, git push --force, git reset --hard,
  DROP TABLE, etc.) unless the user explicitly requests them.
- Before running destructive git operations, consider safer alternatives.
- Never skip git hooks with --no-verify or bypass signing unless the user
  explicitly asks for it.

## Match scope to request
- Only make changes the user asked for. Do not \"improve\" unrelated code.
- If you find an issue outside the scope of the request, mention it but
  do not fix it unless asked.
- When adding error handling, only handle errors that can actually occur
  in the given context.

## Secrets and credentials
- Never commit files that likely contain secrets (.env, credentials.json,
  private keys, API tokens).
- If the user asks you to commit such files, warn them first.
- Do not print secrets to stdout.

## Confirm before large-scale changes
- If a change would touch more than a handful of files, describe what you
  plan to do and ask for confirmation before proceeding.";

/// Coding quality guidelines.
const CODING_GUIDELINES: &str = "\
# Coding Guidelines

## Do not gold-plate
- Do NOT add features, comments, docstrings, or error handling beyond
  what the user asked for.
- Do NOT refactor code that is not directly related to the task.
- Do NOT add type annotations, logging, or abstractions \"for future use\".
- Do NOT create helper functions for operations performed only once.
- Three similar lines of code are better than a premature abstraction.

## Prefer simple solutions
- Write the simplest code that solves the problem correctly.
- Simple is better than clever. Readable is better than compact.
- Avoid unnecessary indirection, generics, or metaprogramming.
- Prefer standard library functions over hand-rolled equivalents.

## File management
- Always prefer editing an existing file over creating a new one.
- Never create new files unless they are strictly necessary.
- When creating files, place them in the conventional location for
  the project's language and framework.

## Code style
- Match the existing style of the codebase (indentation, naming,
  formatting conventions).
- Do not reformat code that you did not change.
- Follow the language's idiomatic conventions (e.g., snake_case in
  Rust and Python, camelCase in JavaScript/TypeScript).
- If there is a formatter configured (.prettierrc, rustfmt.toml, etc.),
  the CI will handle formatting. Do not manually reformat.

## Testing
- When adding new functionality, add tests if the project has a test suite.
- Match the testing style used in the existing tests.
- Do not add tests for trivial code (getters, data classes) unless asked.

## Error handling
- Only add error handling for errors that can actually occur.
- Do not add catch-all handlers for impossible scenarios.
- Propagate errors using the project's established pattern (Result, try/catch,
  error codes, etc.).
- If a function already handles an error case, do not add another handler.

## Dependencies
- Do not add new dependencies unless necessary.
- Prefer the project's existing dependencies over adding new ones.
- If you must add a dependency, choose the most widely-used and
  well-maintained option.";

/// Communication tone and style.
const TONE_AND_STYLE: &str = "\
# Communication Style

- Be concise. Terminal space is limited and the user is a developer.
- Do NOT use emojis unless the user explicitly asks for them.
- When discussing code, reference it as file_path:line_number rather
  than pasting the code back.
- Use owner/repo#123 format when referencing GitHub issues or PRs.
- When summarizing changes, use bullet points or short paragraphs.
- Do not repeat the user's question back to them.
- Do not preface responses with \"Sure!\" or \"Of course!\" or similar
  filler. Go straight to the answer.
- If you made a mistake, acknowledge it directly and fix it. Do not
  make excuses.";

/// Output efficiency rules.
const OUTPUT_EFFICIENCY: &str = "\
# Output Efficiency

- Lead with the answer or action, not the reasoning process.
- Skip preamble, filler, and unnecessary context-setting.
- Focus on: decisions made, status updates, errors encountered,
  and file paths changed.
- When reporting tool results, summarize rather than echoing the
  full output unless the user needs the raw data.
- If a task is complete, say so in one sentence. Do not write a
  multi-paragraph summary of what was obvious.
- Use code blocks only for content the user needs to copy or review.
  Do not wrap prose in code blocks.";

/// Git commit workflow instructions.
const GIT_COMMIT_INSTRUCTIONS: &str = "\
# Git Commit Instructions

When the user asks you to commit changes, follow this protocol:

## Step 1: Gather context (run in parallel)
- git status (to see untracked and modified files; never use -uall)
- git diff (to see staged and unstaged changes)
- git log --oneline -5 (to match the repository's commit message style)

## Step 2: Draft the commit message
- Summarize the nature of the changes (new feature, bug fix, refactor, etc.).
- Use imperative mood (\"Add feature\" not \"Added feature\").
- Keep the first line under 72 characters.
- Add a blank line then a longer description if needed.
- End the message with:
  Co-Authored-By: Claude Code <noreply@anthropic.com>

## Step 3: Stage and commit
- Stage specific files by name. Avoid \"git add -A\" or \"git add .\" which
  can accidentally include secrets or large binaries.
- Use a heredoc to pass the commit message to avoid shell escaping issues.
- Run git status after committing to verify success.

## Important rules
- NEVER push to the remote unless the user explicitly asks.
- NEVER amend a commit unless the user explicitly asks. If a pre-commit
  hook fails, fix the issue, re-stage, and create a NEW commit.
- NEVER skip hooks with --no-verify.
- NEVER force push to main/master. Warn the user if they request this.
- NEVER update the git config.
- If there are no changes to commit, say so. Do not create empty commits.
- Do not commit files that may contain secrets (.env, credentials, keys).";

/// Pull request creation instructions.
const PR_INSTRUCTIONS: &str = "\
# Pull Request Instructions

When the user asks you to create a pull request:

## Step 1: Gather context (run in parallel)
- git status (to check for uncommitted changes)
- git diff (to see any pending changes)
- git log and git diff <base>...HEAD (to understand all commits on the branch)
- Check if the branch tracks a remote and is up to date

## Step 2: Draft the PR
- Keep the PR title short (under 70 characters).
- Use the description body for details.
- Analyze ALL commits on the branch, not just the latest one.

## Step 3: Create the PR
- Push to remote with -u flag if needed.
- Use gh pr create with this format:
  --title \"concise title\"
  --body \"## Summary
  - bullet points

  ## Test plan
  - [ ] testing steps\"

## Important rules
- Only push and create PRs when explicitly asked.
- Include all relevant commits in the description, not just the latest.
- Return the PR URL when done.";

/// Instructions for the auto-memory system.
const MEMORY_INSTRUCTIONS: &str = "\
# Memory System

You have access to a memory system that persists information across sessions.
There are several memory scopes:

- **User memory**: Personal preferences, coding style, common workflows.
- **Project memory**: Repository-specific context like architecture decisions,
  deployment processes, and key file locations.
- **Feedback memory**: Corrections the user has given you that should be
  remembered (e.g., \"always use pnpm, not npm\").

## When to save memories
- When the user tells you a preference (\"I prefer tabs over spaces\").
- When you learn something important about the project structure.
- When the user corrects your behavior and the correction is general.

## When NOT to save memories
- Transient information (\"fix this typo\").
- Information already in the project's config files.
- Obvious facts that do not need to be remembered.";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single section of the system prompt.
#[derive(Debug, Clone)]
pub struct PromptSection {
    /// Human-readable name for this section.
    pub name: String,
    /// The textual content of the section.
    pub content: String,
    /// Priority (lower = placed earlier in the assembled prompt).
    pub priority: u32,
}

/// Git repository context detected from the working directory.
#[derive(Debug, Clone, Default)]
pub struct GitContext {
    /// Current branch name.
    pub branch: String,
    /// Default branch (usually main or master).
    pub default_branch: String,
    /// Short summary of working tree status.
    pub status: String,
    /// Recent commit subjects.
    pub recent_commits: Vec<String>,
    /// Configured user name.
    pub user_name: String,
    /// Configured user email.
    pub user_email: String,
}

/// Information about the runtime environment.
#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    /// Operating system platform (e.g., "win32", "linux", "darwin").
    pub platform: String,
    /// Active shell (e.g., "bash", "zsh", "powershell").
    pub shell: String,
    /// OS version string.
    pub os_version: String,
    /// Absolute path of the working directory.
    pub working_directory: String,
    /// Whether the working directory is inside a git repository.
    pub is_git_repo: bool,
    /// Model name or identifier.
    pub model_name: String,
    /// Current date string (e.g., "2026-03-31").
    pub date: String,
}

// ---------------------------------------------------------------------------
// GitContext detection
// ---------------------------------------------------------------------------

impl GitContext {
    /// Detect git context by running git commands in the given directory.
    ///
    /// Returns `None` if we are not in a git repository or the commands fail.
    pub async fn detect(working_dir: &str) -> Option<Self> {
        // Helper to run a git command and capture stdout.
        async fn git_output(working_dir: &str, args: &[&str]) -> Option<String> {
            let output = tokio::process::Command::new("git")
                .args(args)
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
                .await
                .ok()?;

            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        }

        // Check if we are in a git repo at all.
        git_output(working_dir, &["rev-parse", "--is-inside-work-tree"]).await?;

        // Run remaining queries concurrently.
        let (branch, default_branch, status, log, user_name, user_email) = tokio::join!(
            git_output(working_dir, &["branch", "--show-current"]),
            git_output(working_dir, &["rev-parse", "--abbrev-ref", "origin/HEAD"]),
            git_output(working_dir, &["status", "--short"]),
            git_output(working_dir, &["log", "--oneline", "-10"]),
            git_output(working_dir, &["config", "user.name"]),
            git_output(working_dir, &["config", "user.email"]),
        );

        let recent_commits: Vec<String> = log
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect();

        // Parse default branch from "origin/main" or "origin/master".
        let default_branch = default_branch
            .unwrap_or_default()
            .rsplit('/')
            .next()
            .unwrap_or("main")
            .to_string();

        Some(GitContext {
            branch: branch.unwrap_or_default(),
            default_branch,
            status: status.unwrap_or_default(),
            recent_commits,
            user_name: user_name.unwrap_or_default(),
            user_email: user_email.unwrap_or_default(),
        })
    }
}

// ---------------------------------------------------------------------------
// EnvironmentInfo detection
// ---------------------------------------------------------------------------

impl EnvironmentInfo {
    /// Detect the current environment information.
    pub fn detect(model: &str) -> Self {
        let platform = if cfg!(target_os = "windows") {
            "win32"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        }
        .to_string();

        let shell = std::env::var("SHELL")
            .or_else(|_| std::env::var("COMSPEC"))
            .unwrap_or_else(|_| {
                if cfg!(target_os = "windows") {
                    "cmd.exe".to_string()
                } else {
                    "/bin/sh".to_string()
                }
            });

        let shell_name = shell
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&shell)
            .to_string();

        let os_version = Self::detect_os_version();

        let working_directory = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let is_git_repo = std::process::Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&working_directory)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        EnvironmentInfo {
            platform,
            shell: shell_name,
            os_version,
            working_directory,
            is_git_repo,
            model_name: model.to_string(),
            date,
        }
    }

    fn detect_os_version() -> String {
        #[cfg(target_os = "windows")]
        {
            // Best-effort from environment or generic fallback.
            std::env::var("OS").unwrap_or_else(|_| "Windows".to_string())
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(format!(
                            "macOS {}",
                            String::from_utf8_lossy(&o.stdout).trim()
                        ))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "macOS".to_string())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            std::process::Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(format!(
                            "Linux {}",
                            String::from_utf8_lossy(&o.stdout).trim()
                        ))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Linux".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// SystemPromptBuilder
// ---------------------------------------------------------------------------

/// Assembles a complete system prompt from prioritized sections.
///
/// Each section has a priority; lower numbers appear first in the final
/// prompt. Sections are separated by double newlines.
///
/// # Example
///
/// ```rust
/// use cc_query::system_prompt::{SystemPromptBuilder, EnvironmentInfo};
///
/// let mut builder = SystemPromptBuilder::new();
/// builder.with_identity()
///        .with_tool_instructions()
///        .with_safety_guidelines()
///        .with_coding_guidelines()
///        .with_tone_and_style()
///        .with_output_efficiency();
///
/// let prompt = builder.build();
/// assert!(prompt.contains("Claude Code"));
/// ```
pub struct SystemPromptBuilder {
    sections: Vec<PromptSection>,
}

impl SystemPromptBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Add a custom section with the given name, content, and priority.
    pub fn add_section(&mut self, name: &str, content: &str, priority: u32) {
        self.sections.push(PromptSection {
            name: name.to_string(),
            content: content.to_string(),
            priority,
        });
    }

    /// Concatenate all sections ordered by priority (ascending) into a
    /// single system prompt string.
    pub fn build(&self) -> String {
        let mut sorted: Vec<&PromptSection> = self.sections.iter().collect();
        sorted.sort_by_key(|s| s.priority);

        let mut prompt = String::with_capacity(16 * 1024);
        for (i, section) in sorted.iter().enumerate() {
            if i > 0 {
                prompt.push_str("\n\n");
            }
            prompt.push_str(&section.content);
        }
        prompt
    }

    // -- Static section builders ------------------------------------------

    /// Add the identity section describing who the assistant is.
    pub fn with_identity(&mut self) -> &mut Self {
        self.add_section("identity", IDENTITY, 10);
        self
    }

    /// Add instructions on how to use tools effectively.
    pub fn with_tool_instructions(&mut self) -> &mut Self {
        self.add_section("tool_instructions", TOOL_INSTRUCTIONS, 20);
        self
    }

    /// Add security and safety guidelines.
    pub fn with_safety_guidelines(&mut self) -> &mut Self {
        self.add_section("safety_guidelines", SAFETY_GUIDELINES, 30);
        self
    }

    /// Add coding quality and style guidelines.
    pub fn with_coding_guidelines(&mut self) -> &mut Self {
        self.add_section("coding_guidelines", CODING_GUIDELINES, 40);
        self
    }

    /// Add communication tone and style rules.
    pub fn with_tone_and_style(&mut self) -> &mut Self {
        self.add_section("tone_and_style", TONE_AND_STYLE, 50);
        self
    }

    /// Add output efficiency rules.
    pub fn with_output_efficiency(&mut self) -> &mut Self {
        self.add_section("output_efficiency", OUTPUT_EFFICIENCY, 60);
        self
    }

    /// Add git commit workflow instructions.
    pub fn with_git_commit_instructions(&mut self) -> &mut Self {
        self.add_section("git_commit", GIT_COMMIT_INSTRUCTIONS, 70);
        self
    }

    /// Add pull request creation instructions.
    pub fn with_pr_instructions(&mut self) -> &mut Self {
        self.add_section("pr_instructions", PR_INSTRUCTIONS, 80);
        self
    }

    /// Add memory system instructions.
    pub fn with_memory_instructions(&mut self) -> &mut Self {
        self.add_section("memory_instructions", MEMORY_INSTRUCTIONS, 90);
        self
    }

    // -- Dynamic section builders -----------------------------------------

    /// Add per-tool usage descriptions for the given set of tools.
    pub fn with_tool_descriptions(&mut self, tools: &[ToolDefinition]) -> &mut Self {
        if tools.is_empty() {
            return self;
        }

        let mut content =
            String::from("# Available Tools\n\nYou have access to the following tools:\n");
        for tool in tools {
            let desc = tool_prompts::get_tool_prompt(&tool.name);
            let _ = write!(content, "\n## {}\n{}\n", tool.name, desc);
        }
        self.add_section("tool_descriptions", &content, 25);
        self
    }

    /// Add git repository context.
    pub fn with_git_context(&mut self, git: &GitContext) -> &mut Self {
        let mut content = String::from("# Git Context\n\n");
        let _ = writeln!(content, "Current branch: {}", git.branch);
        let _ = writeln!(content, "Default branch: {}", git.default_branch);

        if !git.user_name.is_empty() {
            let _ = writeln!(content, "Git user: {} <{}>", git.user_name, git.user_email);
        }

        if !git.status.is_empty() {
            let _ = writeln!(content, "\nWorking tree status:\n{}", git.status);
        }

        if !git.recent_commits.is_empty() {
            let _ = writeln!(content, "\nRecent commits:");
            for c in &git.recent_commits {
                let _ = writeln!(content, "  {}", c);
            }
        }

        self.add_section("git_context", &content, 200);
        self
    }

    /// Add user/project memory context.
    pub fn with_memory_context(&mut self, memories: &str) -> &mut Self {
        if memories.is_empty() {
            return self;
        }
        let content = format!(
            "# Remembered Context\n\n\
             The following information was remembered from previous sessions:\n\n\
             {}\n\n\
             Use this context to inform your actions but do not mention it \
             unprompted.",
            memories
        );
        self.add_section("memory_context", &content, 210);
        self
    }

    /// Add environment information.
    pub fn with_environment(&mut self, env: &EnvironmentInfo) -> &mut Self {
        let mut content = String::from("# Environment\n\n");
        let _ = writeln!(content, "Platform: {}", env.platform);
        let _ = writeln!(content, "Shell: {}", env.shell);
        let _ = writeln!(content, "OS: {}", env.os_version);
        let _ = writeln!(content, "Working directory: {}", env.working_directory);
        let _ = writeln!(
            content,
            "Is git repo: {}",
            if env.is_git_repo { "Yes" } else { "No" }
        );
        let _ = writeln!(content, "Model: {}", env.model_name);
        let _ = writeln!(content, "Date: {}", env.date);

        content.push_str(
            "\nIMPORTANT: Use Unix shell syntax (forward slashes, /dev/null) \
                          even on Windows when the shell is bash or similar.",
        );

        self.add_section("environment", &content, 190);
        self
    }

    /// Add plan-mode instructions referencing a specific plan file.
    pub fn with_plan_mode(&mut self, plan_file: &str) -> &mut Self {
        let content = format!(
            "# Plan Mode\n\n\
             You are currently in plan mode. Your plan is stored in: {}\n\n\
             In plan mode:\n\
             - Read the plan file first to understand what has been decided.\n\
             - Break work into small, verifiable steps.\n\
             - Update the plan file as you complete steps.\n\
             - Mark completed items with [x] and pending items with [ ].\n\
             - If you discover new work, add it to the plan before doing it.\n\
             - After finishing each step, re-read the plan to stay oriented.",
            plan_file
        );
        self.add_section("plan_mode", &content, 100);
        self
    }

    // -- Convenience: build the full default prompt -----------------------

    /// Build the standard default system prompt with all static sections
    /// and optionally dynamic sections.
    ///
    /// This is the primary entry point for constructing the system prompt
    /// used by the query loop.
    pub fn build_default(
        tools: &[ToolDefinition],
        env: &EnvironmentInfo,
        git: Option<&GitContext>,
        memory: Option<&str>,
    ) -> String {
        let mut builder = SystemPromptBuilder::new();

        // Static sections (always included).
        builder
            .with_identity()
            .with_tool_instructions()
            .with_safety_guidelines()
            .with_coding_guidelines()
            .with_tone_and_style()
            .with_output_efficiency()
            .with_git_commit_instructions()
            .with_pr_instructions()
            .with_memory_instructions();

        // Dynamic sections.
        if !tools.is_empty() {
            builder.with_tool_descriptions(tools);
        }
        builder.with_environment(env);

        if let Some(git) = git {
            builder.with_git_context(git);
        }
        if let Some(mem) = memory {
            builder.with_memory_context(mem);
        }

        builder.build()
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_identity_contains_claude_code() {
        let mut builder = SystemPromptBuilder::new();
        builder.with_identity();
        let prompt = builder.build();
        assert!(prompt.contains("Claude Code"));
    }

    #[test]
    fn builder_priority_ordering() {
        let mut builder = SystemPromptBuilder::new();
        builder.add_section("low", "LOW_CONTENT", 100);
        builder.add_section("high", "HIGH_CONTENT", 1);
        let prompt = builder.build();
        let high_pos = prompt.find("HIGH_CONTENT").unwrap();
        let low_pos = prompt.find("LOW_CONTENT").unwrap();
        assert!(
            high_pos < low_pos,
            "Higher priority (lower number) should come first"
        );
    }

    #[test]
    fn builder_sections_separated_by_double_newline() {
        let mut builder = SystemPromptBuilder::new();
        builder.add_section("a", "AAA", 1);
        builder.add_section("b", "BBB", 2);
        let prompt = builder.build();
        assert!(prompt.contains("AAA\n\nBBB"));
    }

    #[test]
    fn builder_empty_produces_empty() {
        let builder = SystemPromptBuilder::new();
        assert!(builder.build().is_empty());
    }

    #[test]
    fn default_build_includes_all_static_sections() {
        let env = EnvironmentInfo {
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.1".to_string(),
            working_directory: "/home/user/project".to_string(),
            is_git_repo: true,
            model_name: "claude-sonnet-4-20250514".to_string(),
            date: "2026-03-31".to_string(),
        };

        let prompt = SystemPromptBuilder::build_default(&[], &env, None, None);

        // Check that all main sections are present.
        assert!(prompt.contains("Claude Code"));
        assert!(prompt.contains("Tool Usage Guidelines"));
        assert!(prompt.contains("Safety and Security"));
        assert!(prompt.contains("Coding Guidelines"));
        assert!(prompt.contains("Communication Style"));
        assert!(prompt.contains("Output Efficiency"));
        assert!(prompt.contains("Git Commit Instructions"));
        assert!(prompt.contains("Pull Request Instructions"));
        assert!(prompt.contains("Memory System"));
        assert!(prompt.contains("Environment"));
        assert!(prompt.contains("linux"));
    }

    #[test]
    fn git_context_rendering() {
        let git = GitContext {
            branch: "feature/test".to_string(),
            default_branch: "main".to_string(),
            status: "M src/lib.rs".to_string(),
            recent_commits: vec!["abc1234 Initial commit".to_string()],
            user_name: "Dev".to_string(),
            user_email: "dev@example.com".to_string(),
        };

        let mut builder = SystemPromptBuilder::new();
        builder.with_git_context(&git);
        let prompt = builder.build();

        assert!(prompt.contains("feature/test"));
        assert!(prompt.contains("main"));
        assert!(prompt.contains("Dev <dev@example.com>"));
        assert!(prompt.contains("M src/lib.rs"));
        assert!(prompt.contains("abc1234 Initial commit"));
    }

    #[test]
    fn memory_context_included_when_present() {
        let mut builder = SystemPromptBuilder::new();
        builder.with_memory_context("User prefers tabs over spaces.");
        let prompt = builder.build();
        assert!(prompt.contains("User prefers tabs over spaces."));
    }

    #[test]
    fn memory_context_skipped_when_empty() {
        let mut builder = SystemPromptBuilder::new();
        builder.with_memory_context("");
        let prompt = builder.build();
        assert!(prompt.is_empty());
    }

    #[test]
    fn tool_descriptions_section() {
        let tools = vec![
            ToolDefinition {
                name: "Bash".to_string(),
                description: "Execute shell commands".to_string(),
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "Read".to_string(),
                description: "Read a file".to_string(),
                input_schema: serde_json::json!({}),
            },
        ];

        let mut builder = SystemPromptBuilder::new();
        builder.with_tool_descriptions(&tools);
        let prompt = builder.build();

        assert!(prompt.contains("Available Tools"));
        assert!(prompt.contains("## Bash"));
        assert!(prompt.contains("## Read"));
    }

    #[test]
    fn plan_mode_section() {
        let mut builder = SystemPromptBuilder::new();
        builder.with_plan_mode("/tmp/plan.md");
        let prompt = builder.build();

        assert!(prompt.contains("Plan Mode"));
        assert!(prompt.contains("/tmp/plan.md"));
    }

    #[test]
    fn full_prompt_reasonable_size() {
        let env = EnvironmentInfo {
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.1".to_string(),
            working_directory: "/home/user/project".to_string(),
            is_git_repo: true,
            model_name: "claude-sonnet-4-20250514".to_string(),
            date: "2026-03-31".to_string(),
        };

        let tools = vec![ToolDefinition {
            name: "Bash".to_string(),
            description: "Execute commands".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let git = GitContext {
            branch: "main".to_string(),
            default_branch: "main".to_string(),
            status: String::new(),
            recent_commits: vec!["abc Initial".to_string()],
            user_name: "Dev".to_string(),
            user_email: "dev@test.com".to_string(),
        };

        let prompt = SystemPromptBuilder::build_default(
            &tools,
            &env,
            Some(&git),
            Some("User prefers Rust."),
        );

        // The full prompt should be substantial (3000+ tokens ~ 12000+ chars).
        assert!(
            prompt.len() > 8000,
            "Full prompt should be substantial, got {} chars",
            prompt.len()
        );
        // But not absurdly large.
        assert!(
            prompt.len() < 100_000,
            "Full prompt should not be absurdly large, got {} chars",
            prompt.len()
        );
    }

    #[test]
    fn environment_info_detect() {
        let env = EnvironmentInfo::detect("test-model");
        assert!(!env.platform.is_empty());
        assert!(!env.shell.is_empty());
        assert!(!env.working_directory.is_empty());
        assert!(!env.date.is_empty());
        assert_eq!(env.model_name, "test-model");
    }

    #[test]
    fn test_build_default_produces_output() {
        // Verify that build_default with minimal arguments returns a
        // non-empty string containing the core identity section.
        let env = EnvironmentInfo {
            platform: "test-platform".to_string(),
            shell: "test-shell".to_string(),
            os_version: "TestOS 1.0".to_string(),
            working_directory: "/tmp/test".to_string(),
            is_git_repo: false,
            model_name: "test-model".to_string(),
            date: "2026-01-01".to_string(),
        };

        let prompt = SystemPromptBuilder::build_default(&[], &env, None, None);
        assert!(
            !prompt.is_empty(),
            "build_default should produce non-empty output"
        );
        assert!(
            prompt.len() > 1000,
            "build_default should produce substantial output, got {} chars",
            prompt.len()
        );
        // Core identity must be present.
        assert!(prompt.contains("Claude Code"));
        // Environment section must be present.
        assert!(prompt.contains("test-platform"));
    }

    #[test]
    fn test_environment_info_detect() {
        // Verify EnvironmentInfo::detect fills all fields with reasonable values.
        let env = EnvironmentInfo::detect("claude-sonnet-4-20250514");

        // Platform must be one of the known values.
        assert!(
            ["win32", "darwin", "linux"].contains(&env.platform.as_str()),
            "platform should be win32, darwin, or linux, got: {}",
            env.platform
        );

        // Shell should be a recognizable shell name.
        assert!(!env.shell.is_empty(), "shell should not be empty");

        // OS version should be populated.
        assert!(!env.os_version.is_empty(), "os_version should not be empty");

        // Working directory should be an absolute-ish path.
        assert!(
            !env.working_directory.is_empty(),
            "working_directory should not be empty"
        );

        // Date should look like YYYY-MM-DD.
        assert_eq!(
            env.date.len(),
            10,
            "date should be 10 chars (YYYY-MM-DD), got: {}",
            env.date
        );
        assert!(
            env.date.chars().nth(4) == Some('-'),
            "date should have dash at position 4"
        );

        // Model name should match what we passed in.
        assert_eq!(env.model_name, "claude-sonnet-4-20250514");
    }
}
