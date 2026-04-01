# claude-code-rs

> Claude Code, rewritten in Rust from scratch.

A high-performance, single-binary AI coding assistant for the terminal, built as a Rust reimplementation of [Anthropic's Claude Code](https://docs.anthropic.com/en/docs/claude-code).

## Features

- **Single Binary** — 6MB release build, no runtime dependencies (Node.js/Bun not required)
- **38 Built-in Tools** — Bash, File Read/Write/Edit, Glob, Grep, Agent, MCP, Notebook, Plan Mode, Worktree, and more
- **70 Slash Commands** — `/diff`, `/commit`, `/review`, `/doctor`, `/vim`, `/mcp`, `/resume`, `/export`, and more
- **Streaming API Client** — SSE streaming with exponential backoff retry and full error handling
- **Agentic Loop** — Tool call → execute → loop, with automatic conversation compaction
- **Terminal UI** — ratatui-based REPL with virtual scrolling, diff coloring, search, task panel, permission dialogs
- **Vim Mode** — Full state machine: operators (`d`/`c`/`y`), motions (`w`/`b`/`e`/`f`/`t`), text objects, counts, dot-repeat
- **Permission System** — 4-layer security model (validate → check → rules → canUseTool)
- **MCP Protocol** — JSON-RPC 2.0 with stdio/SSE/HTTP transports and connection manager
- **Bridge** — claude.ai WebSocket bridge with JWT auth, device trust, auto-reconnect
- **Multi-Agent** — Coordinator for worker spawning, permission routing, tool distribution
- **Session Management** — Persist and resume conversations
- **Memory System** — Automatic CLAUDE.md / `.claude/memory/` scanning and context injection
- **Skills & Hooks** — YAML frontmatter skills, pre/post tool hooks with shell execution
- **Plugin System** — Loadable plugins with MCP server integration
- **OAuth PKCE** — Full authentication flow
- **Cost Tracking** — Per-model pricing (Opus/Sonnet/Haiku)

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- [MinGW-w64](https://github.com/niXman/mingw-builds-binaries/releases) (Windows) or GCC (Linux/macOS)
- An [Anthropic API Key](https://console.anthropic.com/)

### Build

```bash
git clone https://github.com/hefuyong/claude-code-rs.git
cd claude-code-rs
cargo build --release
```

The binary will be at `target/release/claude-code` (~6MB).

### Usage

```bash
# Set your API key
export ANTHROPIC_API_KEY="sk-ant-..."

# One-shot mode
claude-code "Explain this codebase"

# Interactive TUI mode
claude-code

# With options
claude-code "Fix the bug" --model claude-opus-4-20250514 --verbose
claude-code "Refactor auth" --max-tokens 32768 --max-turns 20

# JSON output (for piping)
claude-code "List all TODO comments" --print

# System diagnostics
claude-code doctor

# Show configuration
claude-code config

# Resume a previous session
claude-code resume
```

### CLI Reference

```
Usage: claude-code [OPTIONS] [PROMPT] [COMMAND]

Commands:
  config   Show current configuration
  clear    Clear conversation history
  resume   Resume a previous session
  doctor   Run system diagnostics
  mcp      Manage MCP servers
  diff     Show current file changes (git diff)
  version  Show version information

Options:
  -v, --verbose                        Enable verbose output
      --model <MODEL>                  Override model [env: CLAUDE_MODEL]
  -p, --print                          JSON lines output
      --max-tokens <N>                 Max output tokens [default: 16384]
      --max-turns <N>                  Max agentic turns [default: 10]
      --system-prompt <PROMPT>         System prompt override
      --permission-mode <MODE>         default|auto|bypass|plan
      --add-dir <DIR>                  Additional working directories
      --cwd <DIR>                      Override working directory
      --resume <ID>                    Resume session by ID
      --no-memory                      Disable CLAUDE.md loading
      --agent <TYPE>                   Agent type
```

### Slash Commands (Interactive Mode)

| Category | Commands |
|----------|----------|
| **Session** | `/resume` `/session` `/history` `/export` `/save` `/attach` `/detach` `/ps` |
| **Code** | `/diff` `/commit` `/review` `/ultrareview` `/branch` `/pr` `/blame` `/stash` |
| **Config** | `/settings` `/permissions` `/hooks` `/env` `/theme` `/keybindings` |
| **Features** | `/vim` `/voice` `/plan` `/context` `/summary` `/search` `/rewind` `/agents` `/plugin` |
| **Diagnostics** | `/doctor` `/version` `/debug` `/logs` `/tokens` `/benchmark` |
| **MCP** | `/mcp` `/mcp-status` `/skills` `/tools` |
| **Other** | `/help` `/clear` `/exit` `/login` `/logout` `/init` `/update` `/bug` `/feedback` |

## Architecture

31 crates organized as a Cargo workspace:

```
claude-code-rs/
├── src/main.rs              # Binary entry point
├── crates/
│   ├── cc-types/            # Shared type definitions
│   ├── cc-error/            # Error hierarchy (thiserror)
│   ├── cc-config/           # Configuration loading & merging
│   ├── cc-messages/         # Message types & serialization
│   ├── cc-api/              # Anthropic API client (SSE streaming)
│   ├── cc-cost/             # Token counting & cost tracking
│   ├── cc-permissions/      # 4-layer permission model
│   ├── cc-tools-core/       # Tool trait, registry, executor
│   ├── cc-tools/            # 38 tool implementations
│   ├── cc-commands/         # 70 slash commands
│   ├── cc-skills/           # Skill loading (YAML + markdown)
│   ├── cc-hooks/            # Pre/post tool hooks
│   ├── cc-query/            # Agentic loop + system prompts
│   ├── cc-query-engine/     # Session-level query wrapper
│   ├── cc-compact/          # Conversation compaction
│   ├── cc-session/          # Session persistence
│   ├── cc-memory/           # CLAUDE.md scanning
│   ├── cc-state/            # AppState store (tokio watch)
│   ├── cc-tui/              # Terminal UI (ratatui)
│   ├── cc-vim/              # Vim mode state machine
│   ├── cc-mcp/              # MCP protocol (JSON-RPC 2.0)
│   ├── cc-bridge/           # claude.ai WebSocket bridge
│   ├── cc-coordinator/      # Multi-agent orchestration
│   ├── cc-remote/           # Remote session management
│   ├── cc-plugins/          # Plugin system
│   ├── cc-oauth/            # OAuth PKCE flow
│   ├── cc-analytics/        # Telemetry
│   ├── cc-tasks/            # Background task management
│   ├── cc-buddy/            # Companion character system
│   ├── cc-sdk/              # Programmatic SDK interface
│   └── cc-cli/              # CLI parsing & integration
```

### Dependency Graph

```
cc-types, cc-error                    (leaf crates)
  └─> cc-config, cc-messages, cc-permissions
       └─> cc-api, cc-tools-core, cc-state
            └─> cc-query, cc-mcp, cc-session, cc-memory, cc-compact
                 └─> cc-tools, cc-commands, cc-skills, cc-tasks, cc-hooks
                      └─> cc-query-engine, cc-coordinator, cc-bridge, cc-remote
                           └─> cc-tui, cc-sdk
                                └─> cc-cli (top-level binary)
```

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Rust** | Performance, memory safety, single-binary distribution |
| **Cargo workspace** | Incremental compilation, clear dependency boundaries |
| **`async-stream`** | Replaces TypeScript's `AsyncGenerator` for streaming |
| **`tokio::task::JoinSet`** | Parallel tool execution (replaces `Promise.all`) |
| **`tokio::sync::watch`** | Replaces React/Zustand state subscriptions |
| **`ratatui`** | Replaces React/Ink for terminal UI |
| **`thiserror` + `anyhow`** | Typed errors in libraries, flexible errors at boundaries |
| **`#[async_trait]`** | Async methods in the `Tool` trait |

## Configuration

Configuration is loaded from multiple sources (highest priority first):

1. CLI flags (`--model`, `--verbose`, etc.)
2. Environment variables (`ANTHROPIC_API_KEY`, `CLAUDE_MODEL`, `ANTHROPIC_BASE_URL`)
3. Project config (`.claude-code-rs.toml`)
4. Global config (`~/.config/claude-code-rs/config.toml`)
5. Built-in defaults

## Testing

```bash
# Run all 175 tests
cargo test --workspace

# Run specific crate tests
cargo test -p cc-permissions
cargo test -p cc-tools
cargo test -p cc-vim
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | **Required.** Your Anthropic API key |
| `ANTHROPIC_BASE_URL` | API base URL (default: `https://api.anthropic.com`) |
| `CLAUDE_MODEL` | Model override (default: `claude-sonnet-4-20250514`) |

## License

MIT

## Acknowledgements

This project is a Rust reimplementation inspired by [Anthropic's Claude Code](https://docs.anthropic.com/en/docs/claude-code). It is not affiliated with or endorsed by Anthropic.

Built with the help of Claude Opus 4.6.
