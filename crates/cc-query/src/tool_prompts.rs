//! Tool-specific prompt descriptions for Claude Code RS.
//!
//! Each tool recognized by the system has a human-readable description that
//! is injected into the system prompt to teach the model when and how to
//! use that tool. These descriptions supplement the JSON Schema that is
//! sent alongside the tool definitions.

/// Return the system-prompt description for a given tool.
///
/// If the tool name is unknown, a generic fallback is returned.
pub fn get_tool_prompt(tool_name: &str) -> &'static str {
    match tool_name {
        // ── File system tools ───────────────────────────────────────
        "Read" | "read" | "file_read" => READ,
        "Write" | "write" | "file_write" => WRITE,
        "Edit" | "edit" | "file_edit" => EDIT,
        "Glob" | "glob" | "file_glob" => GLOB,
        "Grep" | "grep" | "file_grep" => GREP,

        // ── Shell ───────────────────────────────────────────────────
        "Bash" | "bash" | "shell" => BASH,

        // ── Search and navigation ───────────────────────────────────
        "WebSearch" | "web_search" => WEB_SEARCH,
        "WebFetch" | "web_fetch" => WEB_FETCH,

        // ── Git / GitHub ────────────────────────────────────────────
        "GitStatus" | "git_status" => GIT_STATUS,
        "GitDiff" | "git_diff" => GIT_DIFF,
        "GitLog" | "git_log" => GIT_LOG,
        "GitCommit" | "git_commit" => GIT_COMMIT,
        "GitPush" | "git_push" => GIT_PUSH,
        "GitBranch" | "git_branch" => GIT_BRANCH,
        "GitCheckout" | "git_checkout" => GIT_CHECKOUT,
        "GhPrCreate" | "gh_pr_create" => GH_PR_CREATE,
        "GhPrView" | "gh_pr_view" => GH_PR_VIEW,
        "GhIssueView" | "gh_issue_view" => GH_ISSUE_VIEW,

        // ── Task management ─────────────────────────────────────────
        "TaskCreate" | "task_create" => TASK_CREATE,
        "TaskUpdate" | "task_update" => TASK_UPDATE,
        "TaskList" | "task_list" => TASK_LIST,
        "TaskGet" | "task_get" => TASK_GET,

        // ── Agent orchestration ─────────────────────────────────────
        "Agent" | "agent" | "sub_agent" => AGENT,

        // ── Notebook ────────────────────────────────────────────────
        "NotebookEdit" | "notebook_edit" => NOTEBOOK_EDIT,

        // ── Worktree ────────────────────────────────────────────────
        "EnterWorktree" | "enter_worktree" => ENTER_WORKTREE,
        "ExitWorktree" | "exit_worktree" => EXIT_WORKTREE,

        // ── Cron / scheduling ───────────────────────────────────────
        "CronCreate" | "cron_create" => CRON_CREATE,
        "CronDelete" | "cron_delete" => CRON_DELETE,
        "CronList" | "cron_list" => CRON_LIST,

        // ── Skills ──────────────────────────────────────────────────
        "Skill" | "skill" => SKILL,

        // ── Memory ──────────────────────────────────────────────────
        "MemoryRead" | "memory_read" => MEMORY_READ,
        "MemoryWrite" | "memory_write" => MEMORY_WRITE,

        // ── MCP ─────────────────────────────────────────────────────
        "McpTool" | "mcp_tool" => MCP_TOOL,

        // ── Diff / Patch ────────────────────────────────────────────
        "MultiEdit" | "multi_edit" => MULTI_EDIT,

        // ── Permissions / config ────────────────────────────────────
        "PermissionRequest" | "permission_request" => PERMISSION_REQUEST,

        // ── Diagnostics ─────────────────────────────────────────────
        "Diagnostics" | "diagnostics" => DIAGNOSTICS,
        "ListDir" | "list_dir" => LIST_DIR,
        "TodoWrite" | "todo_write" => TODO_WRITE,

        // ── Fallback ────────────────────────────────────────────────
        _ => UNKNOWN_TOOL,
    }
}

// ---------------------------------------------------------------------------
// File system tools
// ---------------------------------------------------------------------------

const READ: &str = "\
Reads the contents of a file from the local filesystem.
- The file_path must be an absolute path, never relative.
- By default reads up to 2000 lines from the start of the file.
- For large files, use the offset and limit parameters to read specific
  ranges rather than the entire file.
- Can read images (PNG, JPG, etc.) -- contents are presented visually.
- Can read PDF files. For large PDFs (>10 pages), provide a pages parameter
  (e.g., \"1-5\"). Maximum 20 pages per request.
- Can read Jupyter notebooks (.ipynb) and returns all cells with outputs.
- Cannot read directories. Use Bash with `ls` to list directory contents.
- If a file exists but is empty, you will receive a warning.
- Always use this tool to view screenshots the user provides.";

const WRITE: &str = "\
Writes content to a file on the local filesystem.
- Overwrites the existing file if one exists at the given path.
- You MUST use the Read tool first before overwriting an existing file.
  The tool will fail if you have not read it in this conversation.
- Prefer the Edit tool for modifying existing files -- it only sends the
  diff and is more efficient.
- Only use Write to create brand-new files or when a complete rewrite is needed.
- Never create documentation files (*.md, README) unless explicitly asked.
- Do not use emojis in file content unless the user requests it.";

const EDIT: &str = "\
Performs exact string replacements in files.
- You must have used Read on the file earlier in this conversation.
- The old_string must be unique in the file. If it is not, provide more
  surrounding context to make it unique, or use replace_all.
- Preserve exact indentation (tabs/spaces) as shown in the Read output.
  The line-number prefix from Read is NOT part of the file content.
- Use replace_all=true to rename variables or change all occurrences
  of a string.
- Always prefer Edit over Write for existing files.
- Do not use emojis unless the user explicitly requests it.";

const GLOB: &str = "\
Fast file pattern matching tool.
- Supports glob patterns like \"**/*.rs\", \"src/**/*.ts\", \"*.json\".
- Returns matching file paths sorted by modification time (newest first).
- Use this instead of `find` or `ls` via Bash.
- Good for discovering files by name pattern when you do not know where
  something lives.
- For content-based searches, use Grep instead.";

const GREP: &str = "\
Content search tool built on ripgrep.
- Supports full regex syntax (e.g., \"fn\\s+\\w+\", \"TODO.*fix\").
- Output modes: \"files_with_matches\" (default, shows paths),
  \"content\" (shows matching lines), \"count\" (shows match counts).
- Filter by file glob (\"*.rs\") or type (\"rust\", \"py\", \"js\").
- Use -A/-B/-C for context lines around matches.
- Use multiline: true for patterns spanning multiple lines.
- Use head_limit to cap output size (default 250).
- Always use this tool instead of grep or rg via Bash.";

// ---------------------------------------------------------------------------
// Shell
// ---------------------------------------------------------------------------

const BASH: &str = "\
Executes a bash command and returns its output.
- The working directory persists between commands, but shell state
  (variables, aliases) does not.
- Use absolute paths. Avoid `cd` unless explicitly needed.
- Quote file paths containing spaces with double quotes.
- Use Unix shell syntax even on Windows when the shell is bash.
- Avoid using this for tasks where a dedicated tool exists (Read, Edit,
  Write, Glob, Grep).
- Use run_in_background for long-running commands.
- Optional timeout parameter (max 600000ms / 10 minutes).
- For git commands, prefer creating new commits over amending.
- Never run destructive commands (rm -rf, git push --force, etc.)
  unless the user explicitly asks.";

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

const WEB_SEARCH: &str = "\
Searches the web and returns results with links.
- Provides up-to-date information beyond the model's training cutoff.
- After answering, include a Sources section with markdown hyperlinks.
- Support domain filtering with allowed_domains/blocked_domains.
- Use the current year (2026) when searching for recent information.";

const WEB_FETCH: &str = "\
Fetches content from a URL and processes it with a prompt.
- Takes a URL and a prompt describing what to extract.
- Converts HTML to markdown and processes with a small, fast model.
- Will NOT work for authenticated or private URLs (Google Docs, Jira, etc.).
- HTTP URLs are automatically upgraded to HTTPS.
- Includes a 15-minute cache.
- For GitHub URLs, prefer gh CLI via Bash instead.";

// ---------------------------------------------------------------------------
// Git / GitHub
// ---------------------------------------------------------------------------

const GIT_STATUS: &str = "\
Shows the working tree status (staged, unstaged, untracked files).
- Never use the -uall flag as it can cause memory issues on large repos.
- Use this before committing to understand what will be included.";

const GIT_DIFF: &str = "\
Shows changes between commits, the working tree, and the index.
- Use to review staged and unstaged changes before committing.
- Use with a branch range (base...HEAD) to see all changes on a branch.";

const GIT_LOG: &str = "\
Shows the commit history.
- Use --oneline for compact output.
- Use -N to limit the number of commits shown.
- Useful for understanding commit message style before creating new commits.";

const GIT_COMMIT: &str = "\
Creates a new git commit.
- Always run git status and git diff first to understand what will be committed.
- Stage specific files rather than using git add -A.
- Use a heredoc for the commit message to avoid escaping issues.
- End the message with the Co-Authored-By line.
- Never amend unless the user explicitly asks.";

const GIT_PUSH: &str = "\
Pushes commits to the remote repository.
- ONLY use when the user explicitly asks to push.
- Never force push to main/master without explicit confirmation.
- Use -u to set the upstream tracking branch when pushing a new branch.";

const GIT_BRANCH: &str = "\
Lists, creates, or deletes branches.
- Use -a to list all branches including remote-tracking ones.
- Never delete branches without explicit user confirmation.";

const GIT_CHECKOUT: &str = "\
Switches branches or restores working tree files.
- Be cautious: checkout can discard uncommitted changes.
- Prefer creating a new branch rather than discarding work.
- Consider whether there is a safer alternative before running.";

const GH_PR_CREATE: &str = "\
Creates a pull request using the GitHub CLI.
- Keep the title under 70 characters.
- Include a Summary section with bullet points and a Test Plan section.
- Push to the remote first if needed.
- Return the PR URL when done.";

const GH_PR_VIEW: &str = "\
Views pull request details using the GitHub CLI.
- Shows title, description, status, reviewers, and checks.
- Use gh api for accessing PR comments.";

const GH_ISSUE_VIEW: &str = "\
Views issue details using the GitHub CLI.
- Shows title, description, labels, assignees, and status.";

// ---------------------------------------------------------------------------
// Task management
// ---------------------------------------------------------------------------

const TASK_CREATE: &str = "\
Creates a tracked task in the session task list.
- Use for complex multi-step work (3+ distinct steps).
- Write a brief, actionable subject in imperative form.
- Include a description of what needs to be done.
- Optional activeForm for the spinner text (present continuous).
- Do NOT use for trivial single-step tasks.
- After creating tasks, set up dependencies with TaskUpdate if needed.";

const TASK_UPDATE: &str = "\
Updates a task's status, description, or dependencies.
- Set status to in_progress before starting work on a task.
- Set status to completed only after fully finishing the work.
- Set status to deleted to permanently remove a task.
- Use addBlockedBy/addBlocks to define task dependencies.
- Do NOT mark a task completed if tests fail or work is partial.
- After completing a task, use TaskList to find the next one.";

const TASK_LIST: &str = "\
Lists all tasks in the current session.
- Shows id, subject, status, owner, and blockedBy for each task.
- Use to find available work (pending, unblocked tasks).
- Prefer working on tasks in ID order (lowest first).
- Use TaskGet with a specific ID for full details.";

const TASK_GET: &str = "\
Retrieves full details of a single task by ID.
- Returns subject, description, status, blocks, and blockedBy.
- Use before starting work to understand full requirements.
- Check that blockedBy is empty before beginning work.";

// ---------------------------------------------------------------------------
// Agent orchestration
// ---------------------------------------------------------------------------

const AGENT: &str = "\
Spawns a sub-agent to handle a complex, multi-step research task.
- The sub-agent has access to the same tools and can search, read, and
  analyze code independently.
- Use for open-ended investigations that may require multiple rounds
  of searching and reading.
- Provide a clear, specific prompt describing what to investigate.
- The sub-agent returns a summary when done.
- Prefer Glob/Grep for simple searches; use Agent for complex ones.";

// ---------------------------------------------------------------------------
// Notebook
// ---------------------------------------------------------------------------

const NOTEBOOK_EDIT: &str = "\
Edits a Jupyter notebook (.ipynb) cell.
- The notebook_path must be absolute.
- cell_number is 0-indexed.
- edit_mode: \"replace\" (default), \"insert\" (add new cell), \"delete\".
- For insert, specify cell_type (\"code\" or \"markdown\").
- Use cell_id to target a specific cell by its ID.";

// ---------------------------------------------------------------------------
// Worktree
// ---------------------------------------------------------------------------

const ENTER_WORKTREE: &str = "\
Creates an isolated git worktree and switches the session into it.
- ONLY use when the user explicitly says \"worktree\".
- Creates a new branch based on HEAD inside .claude/worktrees/.
- Do NOT use when the user simply asks to create or switch branches.
- Use ExitWorktree to leave.";

const EXIT_WORKTREE: &str = "\
Exits a worktree session created by EnterWorktree.
- Only operates on worktrees created in this session.
- action: \"keep\" preserves the worktree, \"remove\" deletes it.
- With \"remove\", use discard_changes: true if there are uncommitted changes
  (the tool will refuse and list them otherwise).
- Only call when the user asks to exit the worktree.";

// ---------------------------------------------------------------------------
// Cron / scheduling
// ---------------------------------------------------------------------------

const CRON_CREATE: &str = "\
Schedules a prompt to run at a future time or on a recurring basis.
- Uses standard 5-field cron syntax (minute hour day-of-month month day-of-week)
  in the user's local timezone.
- Set recurring: false for one-shot reminders (auto-deleted after firing).
- Avoid the :00 and :30 minute marks when the task allows it, to spread load.
- Set durable: true only when the user asks for the task to persist across
  sessions. Most one-shot reminders should stay session-only.
- Recurring tasks auto-expire after 7 days.";

const CRON_DELETE: &str = "\
Cancels a scheduled cron job by its ID.
- Pass the ID returned by CronCreate.
- Removes from both durable storage and in-memory session store.";

const CRON_LIST: &str = "\
Lists all scheduled cron jobs (both durable and session-only).
- Shows job IDs, cron expressions, and prompts.
- Use to check what recurring tasks are active.";

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

const SKILL: &str = "\
Invokes a named skill (slash command) within the conversation.
- Skills provide specialized capabilities and domain knowledge.
- When users reference \"/<something>\", they are referring to a skill.
- Use this tool to invoke it BEFORE generating other responses.
- Examples: skill: \"commit\", skill: \"review-pr\", args: \"123\".
- Do not invoke a skill that is already running.
- Do not use for built-in CLI commands (/help, /clear, etc.).";

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

const MEMORY_READ: &str = "\
Reads stored memories from a specified scope.
- Scopes: user, project, feedback.
- Returns key-value pairs of remembered information.
- Use at the start of a session to recall user preferences.";

const MEMORY_WRITE: &str = "\
Writes a memory entry to a specified scope.
- Scopes: user, project, feedback.
- Save preferences (\"User prefers tabs\"), project context (\"API uses REST\"),
  or behavioral corrections the user has given you.
- Do not save transient information or data already in config files.";

// ---------------------------------------------------------------------------
// MCP
// ---------------------------------------------------------------------------

const MCP_TOOL: &str = "\
Invokes a tool provided by an MCP (Model Context Protocol) server.
- MCP servers extend the tool set with external capabilities.
- Prefer MCP-provided tools over built-in equivalents when available,
  as they may have fewer restrictions or more features.
- The tool name is prefixed with the MCP server name.";

// ---------------------------------------------------------------------------
// Multi-edit / Diff
// ---------------------------------------------------------------------------

const MULTI_EDIT: &str = "\
Applies multiple edits to one or more files in a single operation.
- More efficient than multiple individual Edit calls when making
  several related changes.
- Each edit is an exact string replacement, same rules as Edit.
- All edits are applied atomically.";

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

const PERMISSION_REQUEST: &str = "\
Requests permission from the user for a sensitive operation.
- Use when an operation requires explicit user approval.
- Describe clearly what you want to do and why.
- Wait for the user's response before proceeding.";

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

const DIAGNOSTICS: &str = "\
Retrieves diagnostic information about the current session.
- Shows system status, active tools, memory usage, and configuration.
- Useful for debugging issues with the environment or tool execution.";

const LIST_DIR: &str = "\
Lists the contents of a directory.
- Returns file and directory names with basic metadata.
- Use for exploring the file structure of a project.
- For file-pattern searches, prefer Glob instead.";

const TODO_WRITE: &str = "\
Writes or updates TODO items in the project.
- Use TaskCreate/TaskUpdate instead for in-session task tracking.
- This tool writes persistent TODO markers into files.
- Only use when the user specifically asks for TODOs in code.";

// ---------------------------------------------------------------------------
// Fallback
// ---------------------------------------------------------------------------

const UNKNOWN_TOOL: &str = "\
This tool does not have a detailed prompt description.
Use the tool's JSON Schema and name to understand its purpose.";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tools_return_specific_prompt() {
        let tools = [
            "Read",
            "Write",
            "Edit",
            "Glob",
            "Grep",
            "Bash",
            "WebSearch",
            "WebFetch",
            "TaskCreate",
            "TaskUpdate",
            "TaskList",
            "TaskGet",
            "Agent",
            "NotebookEdit",
            "EnterWorktree",
            "ExitWorktree",
            "CronCreate",
            "CronDelete",
            "CronList",
            "Skill",
            "MemoryRead",
            "MemoryWrite",
            "McpTool",
            "MultiEdit",
            "PermissionRequest",
            "Diagnostics",
            "ListDir",
            "TodoWrite",
            "GitStatus",
            "GitDiff",
            "GitLog",
            "GitCommit",
            "GitPush",
            "GitBranch",
            "GitCheckout",
            "GhPrCreate",
            "GhPrView",
            "GhIssueView",
        ];

        for tool in &tools {
            let prompt = get_tool_prompt(tool);
            assert_ne!(
                prompt, UNKNOWN_TOOL,
                "Tool '{}' should have a specific prompt",
                tool
            );
            assert!(
                prompt.len() > 50,
                "Tool '{}' prompt should be substantive (got {} chars)",
                tool,
                prompt.len()
            );
        }
    }

    #[test]
    fn lowercase_aliases_work() {
        assert_ne!(get_tool_prompt("read"), UNKNOWN_TOOL);
        assert_ne!(get_tool_prompt("write"), UNKNOWN_TOOL);
        assert_ne!(get_tool_prompt("edit"), UNKNOWN_TOOL);
        assert_ne!(get_tool_prompt("glob"), UNKNOWN_TOOL);
        assert_ne!(get_tool_prompt("grep"), UNKNOWN_TOOL);
        assert_ne!(get_tool_prompt("bash"), UNKNOWN_TOOL);
    }

    #[test]
    fn unknown_tool_returns_fallback() {
        assert_eq!(get_tool_prompt("nonexistent_tool"), UNKNOWN_TOOL);
    }

    #[test]
    fn all_prompts_are_non_empty() {
        let all_prompts = [
            READ,
            WRITE,
            EDIT,
            GLOB,
            GREP,
            BASH,
            WEB_SEARCH,
            WEB_FETCH,
            GIT_STATUS,
            GIT_DIFF,
            GIT_LOG,
            GIT_COMMIT,
            GIT_PUSH,
            GIT_BRANCH,
            GIT_CHECKOUT,
            GH_PR_CREATE,
            GH_PR_VIEW,
            GH_ISSUE_VIEW,
            TASK_CREATE,
            TASK_UPDATE,
            TASK_LIST,
            TASK_GET,
            AGENT,
            NOTEBOOK_EDIT,
            ENTER_WORKTREE,
            EXIT_WORKTREE,
            CRON_CREATE,
            CRON_DELETE,
            CRON_LIST,
            SKILL,
            MEMORY_READ,
            MEMORY_WRITE,
            MCP_TOOL,
            MULTI_EDIT,
            PERMISSION_REQUEST,
            DIAGNOSTICS,
            LIST_DIR,
            TODO_WRITE,
        ];

        for (i, prompt) in all_prompts.iter().enumerate() {
            assert!(!prompt.is_empty(), "Prompt at index {} is empty", i);
        }
    }

    #[test]
    fn count_distinct_tools() {
        // Verify we cover at least 38 distinct tool constant entries.
        let all_prompts: Vec<&str> = vec![
            READ,
            WRITE,
            EDIT,
            GLOB,
            GREP,
            BASH,
            WEB_SEARCH,
            WEB_FETCH,
            GIT_STATUS,
            GIT_DIFF,
            GIT_LOG,
            GIT_COMMIT,
            GIT_PUSH,
            GIT_BRANCH,
            GIT_CHECKOUT,
            GH_PR_CREATE,
            GH_PR_VIEW,
            GH_ISSUE_VIEW,
            TASK_CREATE,
            TASK_UPDATE,
            TASK_LIST,
            TASK_GET,
            AGENT,
            NOTEBOOK_EDIT,
            ENTER_WORKTREE,
            EXIT_WORKTREE,
            CRON_CREATE,
            CRON_DELETE,
            CRON_LIST,
            SKILL,
            MEMORY_READ,
            MEMORY_WRITE,
            MCP_TOOL,
            MULTI_EDIT,
            PERMISSION_REQUEST,
            DIAGNOSTICS,
            LIST_DIR,
            TODO_WRITE,
        ];
        assert!(
            all_prompts.len() >= 38,
            "Expected at least 38 tool prompts, got {}",
            all_prompts.len()
        );
    }
}
