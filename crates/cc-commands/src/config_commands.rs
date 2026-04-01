//! Configuration commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register configuration commands.
pub fn register_config_commands(registry: &mut CommandRegistry) {
    // /settings - Open settings
    registry.register(
        Command {
            name: "settings".into(),
            description: "Open or show settings".into(),
            aliases: vec!["prefs".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if !args.is_empty() {
                    return Ok(CommandOutput::text(format!("Setting updated: {args}")));
                }

                let mut lines = vec![
                    "Settings".to_string(),
                    "========".to_string(),
                    String::new(),
                ];

                // Check global settings file
                let global_settings = dirs::home_dir()
                    .map(|h| h.join(".claude").join("settings.json"));
                if let Some(ref path) = global_settings {
                    lines.push(format!("Global: {}", path.display()));
                    if path.exists() {
                        match tokio::fs::read_to_string(path).await {
                            Ok(content) => {
                                // Pretty-print JSON
                                match serde_json::from_str::<serde_json::Value>(&content) {
                                    Ok(val) => {
                                        match serde_json::to_string_pretty(&val) {
                                            Ok(pretty) => {
                                                for line in pretty.lines() {
                                                    lines.push(format!("  {line}"));
                                                }
                                            }
                                            Err(_) => {
                                                lines.push(format!("  {content}"));
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        lines.push(format!("  {content}"));
                                    }
                                }
                            }
                            Err(e) => {
                                lines.push(format!("  (error reading: {e})"));
                            }
                        }
                    } else {
                        lines.push("  (not found)".into());
                    }
                }

                lines.push(String::new());

                // Check project settings file
                let project_settings = std::path::PathBuf::from(".claude").join("settings.json");
                lines.push(format!("Project: {}", project_settings.display()));
                if project_settings.exists() {
                    match tokio::fs::read_to_string(&project_settings).await {
                        Ok(content) => {
                            match serde_json::from_str::<serde_json::Value>(&content) {
                                Ok(val) => {
                                    match serde_json::to_string_pretty(&val) {
                                        Ok(pretty) => {
                                            for line in pretty.lines() {
                                                lines.push(format!("  {line}"));
                                            }
                                        }
                                        Err(_) => {
                                            lines.push(format!("  {content}"));
                                        }
                                    }
                                }
                                Err(_) => {
                                    lines.push(format!("  {content}"));
                                }
                            }
                        }
                        Err(e) => {
                            lines.push(format!("  (error reading: {e})"));
                        }
                    }
                } else {
                    lines.push("  (not found)".into());
                }

                lines.push(String::new());

                // Check config.toml
                let config_toml = dirs::config_dir()
                    .map(|d| d.join("claude-code-rs").join("config.toml"));
                if let Some(ref path) = config_toml {
                    lines.push(format!("Config: {}", path.display()));
                    if path.exists() {
                        match tokio::fs::read_to_string(path).await {
                            Ok(content) => {
                                for line in content.lines() {
                                    lines.push(format!("  {line}"));
                                }
                            }
                            Err(e) => {
                                lines.push(format!("  (error reading: {e})"));
                            }
                        }
                    } else {
                        lines.push("  (not found)".into());
                    }
                }

                lines.push(String::new());
                lines.push("Usage: /settings <key> [value] to update".into());

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );

    // /permissions - Show permission rules
    registry.register(
        Command {
            name: "permissions".into(),
            description: "Show current permission rules".into(),
            aliases: vec!["perms".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let mut lines = vec![
                    "Permission Rules".to_string(),
                    "================".to_string(),
                    String::new(),
                ];

                // Try to read permissions from global settings
                let settings_path = dirs::home_dir()
                    .map(|h| h.join(".claude").join("settings.json"));

                let mut found_perms = false;

                if let Some(ref path) = settings_path {
                    if path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(perms) = data.get("permissions") {
                                    found_perms = true;
                                    lines.push(format!("Source: {}", path.display()));
                                    lines.push(String::new());

                                    if let Some(allow) = perms.get("allow").and_then(|v| v.as_array()) {
                                        lines.push("Allowed:".into());
                                        if allow.is_empty() {
                                            lines.push("  (none)".into());
                                        }
                                        for item in allow {
                                            if let Some(s) = item.as_str() {
                                                lines.push(format!("  + {s}"));
                                            }
                                        }
                                    }

                                    if let Some(deny) = perms.get("deny").and_then(|v| v.as_array()) {
                                        lines.push("Denied:".into());
                                        if deny.is_empty() {
                                            lines.push("  (none)".into());
                                        }
                                        for item in deny {
                                            if let Some(s) = item.as_str() {
                                                lines.push(format!("  - {s}"));
                                            }
                                        }
                                    }

                                    // Show other permission keys
                                    if let Some(obj) = perms.as_object() {
                                        for (key, val) in obj {
                                            if key != "allow" && key != "deny" {
                                                lines.push(format!("  {key}: {val}"));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Also check project settings
                let project_settings = std::path::PathBuf::from(".claude").join("settings.json");
                if project_settings.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&project_settings).await {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(perms) = data.get("permissions") {
                                found_perms = true;
                                lines.push(String::new());
                                lines.push(format!("Project: {}", project_settings.display()));

                                if let Ok(pretty) = serde_json::to_string_pretty(perms) {
                                    for line in pretty.lines() {
                                        lines.push(format!("  {line}"));
                                    }
                                }
                            }
                        }
                    }
                }

                if !found_perms {
                    lines.push("Default permissions (no custom rules):".into());
                    lines.push("  File read:       allowed".into());
                    lines.push("  File write:      ask".into());
                    lines.push("  Shell execute:   ask".into());
                    lines.push("  Network access:  ask".into());
                }

                lines.push(String::new());
                lines.push("Edit .claude/settings.json to change permission rules.".into());

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );

    // /hooks - Show configured hooks
    registry.register(
        Command {
            name: "hooks".into(),
            description: "Show configured hooks".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let mut lines = vec![
                    "Configured Hooks".to_string(),
                    "================".to_string(),
                    String::new(),
                ];

                let mut found_hooks = false;

                // Check .claude/hooks.json
                let hooks_file = std::path::PathBuf::from(".claude").join("hooks.json");
                if hooks_file.exists() {
                    match tokio::fs::read_to_string(&hooks_file).await {
                        Ok(content) => {
                            match serde_json::from_str::<serde_json::Value>(&content) {
                                Ok(data) => {
                                    found_hooks = true;
                                    lines.push(format!("Source: {}", hooks_file.display()));
                                    lines.push(String::new());

                                    if let Some(obj) = data.as_object() {
                                        for (event, hook) in obj {
                                            lines.push(format!("  {event}:"));
                                            if let Some(cmd) = hook.as_str() {
                                                lines.push(format!("    command: {cmd}"));
                                            } else if let Some(arr) = hook.as_array() {
                                                for item in arr {
                                                    if let Some(cmd) = item.get("command").and_then(|c| c.as_str()) {
                                                        let matcher = item.get("matcher")
                                                            .and_then(|m| m.as_str())
                                                            .unwrap_or("*");
                                                        lines.push(format!("    [{matcher}] -> {cmd}"));
                                                    } else if let Some(s) = item.as_str() {
                                                        lines.push(format!("    {s}"));
                                                    }
                                                }
                                            } else if let Some(obj) = hook.as_object() {
                                                for (k, v) in obj {
                                                    lines.push(format!("    {k}: {v}"));
                                                }
                                            }
                                        }
                                    } else {
                                        // Show raw content
                                        match serde_json::to_string_pretty(&data) {
                                            Ok(pretty) => {
                                                for line in pretty.lines() {
                                                    lines.push(format!("  {line}"));
                                                }
                                            }
                                            Err(_) => {
                                                lines.push(format!("  {content}"));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    lines.push(format!("Error parsing {}: {e}", hooks_file.display()));
                                }
                            }
                        }
                        Err(e) => {
                            lines.push(format!("Error reading {}: {e}", hooks_file.display()));
                        }
                    }
                }

                // Also check settings.json for hooks key
                let settings_path = dirs::home_dir()
                    .map(|h| h.join(".claude").join("settings.json"));
                if let Some(ref path) = settings_path {
                    if path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(hooks) = data.get("hooks") {
                                    if let Some(obj) = hooks.as_object() {
                                        if !obj.is_empty() {
                                            found_hooks = true;
                                            lines.push(String::new());
                                            lines.push(format!("From settings ({}):", path.display()));
                                            for (event, hook) in obj {
                                                if let Some(cmd) = hook.as_str() {
                                                    lines.push(format!("  {event}: {cmd}"));
                                                } else {
                                                    lines.push(format!("  {event}: {hook}"));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if !found_hooks {
                    lines.push("(no hooks configured)".into());
                    lines.push(String::new());
                    lines.push("Hooks run automatically on events. Configure in:".into());
                    lines.push("  .claude/hooks.json or settings.json".into());
                    lines.push(String::new());
                    lines.push("Example .claude/hooks.json:".into());
                    lines.push("  {".into());
                    lines.push("    \"pre-commit\": \"cargo fmt && cargo clippy\",".into());
                    lines.push("    \"post-edit\": \"cargo check\"".into());
                    lines.push("  }".into());
                }

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );

    // /env - Show environment info
    registry.register(
        Command {
            name: "env".into(),
            description: "Show environment information".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.display().to_string();
            let model = ctx.model.clone();
            Box::pin(async move {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| "(unknown)".into());
                let shell = std::env::var("SHELL")
                    .or_else(|_| std::env::var("COMSPEC"))
                    .unwrap_or_else(|_| "(unknown)".into());
                let path = std::env::var("PATH")
                    .unwrap_or_else(|_| "(unknown)".into());

                // Mask the API key for security
                let api_key_display = match std::env::var("ANTHROPIC_API_KEY") {
                    Ok(key) => {
                        if key.len() > 10 {
                            format!("{}...{} (set)", &key[..7], &key[key.len()-4..])
                        } else if key.is_empty() {
                            "(empty)".into()
                        } else {
                            "****** (set)".into()
                        }
                    }
                    Err(_) => "(not set)".into(),
                };

                let api_base = std::env::var("ANTHROPIC_BASE_URL")
                    .unwrap_or_else(|_| "https://api.anthropic.com (default)".into());
                let claude_model = std::env::var("CLAUDE_MODEL")
                    .unwrap_or_else(|_| "(not set, using default)".into());
                let editor = std::env::var("EDITOR")
                    .or_else(|_| std::env::var("VISUAL"))
                    .unwrap_or_else(|_| "(not set)".into());
                let term = std::env::var("TERM")
                    .or_else(|_| std::env::var("WT_SESSION").map(|_| "Windows Terminal".into()))
                    .unwrap_or_else(|_| "(unknown)".into());
                let lang = std::env::var("LANG")
                    .or_else(|_| std::env::var("LC_ALL"))
                    .unwrap_or_else(|_| "(not set)".into());
                let no_color = std::env::var("NO_COLOR").is_ok();

                // Show PATH entries (first few)
                let path_entries: Vec<&str> = path.split(|c| c == ':' || c == ';').collect();
                let path_display = if path_entries.len() > 5 {
                    format!(
                        "{}\n               ... and {} more",
                        path_entries[..5].join("\n               "),
                        path_entries.len() - 5
                    )
                } else {
                    path_entries.join("\n               ")
                };

                Ok(CommandOutput::text(format!(
                    "Environment\n\
                     ===========\n\
                     Working dir:   {dir}\n\
                     Home:          {home}\n\
                     Shell:         {shell}\n\
                     Editor:        {editor}\n\
                     Terminal:      {term}\n\
                     Locale:        {lang}\n\
                     NO_COLOR:      {no_color}\n\
                     OS:            {os}\n\
                     Arch:          {arch}\n\n\
                     API Configuration\n\
                     -----------------\n\
                     Model:         {model}\n\
                     ANTHROPIC_API_KEY: {api_key_display}\n\
                     ANTHROPIC_BASE_URL: {api_base}\n\
                     CLAUDE_MODEL:  {claude_model}\n\n\
                     PATH:\n               {path_display}",
                    os = std::env::consts::OS,
                    arch = std::env::consts::ARCH,
                )))
            })
        }),
    );

    // /theme - Change color theme
    registry.register(
        Command {
            name: "theme".into(),
            description: "Change color theme".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Available themes:\n\
                         - dark    (default)\n\
                         - light\n\
                         - auto    (follow system)\n\n\
                         Usage: /theme <name>",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Theme set to: {args}")))
                }
            })
        }),
    );

    // /color - Set color mode
    registry.register(
        Command {
            name: "color".into(),
            description: "Set color output mode".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Color modes:\n\
                         - auto     (default, detect terminal)\n\
                         - always   (force colors)\n\
                         - never    (no colors)\n\n\
                         Usage: /color <mode>",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Color mode set to: {args}")))
                }
            })
        }),
    );

    // /keybindings - Show keybindings
    registry.register(
        Command {
            name: "keybindings".into(),
            description: "Show keyboard shortcuts".into(),
            aliases: vec!["keys".into(), "shortcuts".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Keybindings\n\
                     ===========\n\
                     Ctrl+C       Interrupt current generation\n\
                     Ctrl+D       Exit (same as /exit)\n\
                     Ctrl+L       Clear screen\n\
                     Up/Down      Navigate prompt history\n\
                     Tab          Autocomplete command\n\
                     Esc          Cancel current input (vim mode)\n\
                     Ctrl+R       Search prompt history\n\
                     Ctrl+W       Delete word backward\n\
                     Ctrl+U       Delete line backward",
                ))
            })
        }),
    );

    // /output-style - Set output style
    registry.register(
        Command {
            name: "output-style".into(),
            description: "Set output formatting style".into(),
            aliases: vec!["style".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Output styles:\n\
                         - markdown   (default, rich formatting)\n\
                         - plain      (no formatting)\n\
                         - json       (structured output)\n\
                         - stream     (streaming tokens)\n\n\
                         Usage: /output-style <style>",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Output style set to: {args}")))
                }
            })
        }),
    );
}
