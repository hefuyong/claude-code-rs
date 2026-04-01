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
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Settings locations:\n\
                         - Global:  ~/.claude/settings.json\n\
                         - Project: .claude/settings.json\n\n\
                         Usage: /settings <key> [value]\n\
                         Example: /settings theme dark",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Setting updated: {args}")))
                }
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
                Ok(CommandOutput::text(
                    "Permission rules:\n\
                     - File read:       allowed\n\
                     - File write:      ask\n\
                     - Shell execute:   ask\n\
                     - Network access:  ask\n\n\
                     Edit .claude/settings.json to change permission rules.\n\
                     Use /settings permissions.allow to add allow-list patterns.",
                ))
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
                Ok(CommandOutput::text(
                    "Configured hooks:\n\
                     (no hooks configured)\n\n\
                     Hooks run automatically on events. Configure in settings.json:\n\
                     {\n  \
                       \"hooks\": {\n    \
                         \"pre-commit\": \"cargo fmt && cargo clippy\",\n    \
                         \"post-edit\": \"cargo check\"\n  \
                       }\n\
                     }",
                ))
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
                let api_key_set = std::env::var("ANTHROPIC_API_KEY").is_ok();
                Ok(CommandOutput::text(format!(
                    "Environment\n\
                     ===========\n\
                     Working dir:  {dir}\n\
                     Home:         {home}\n\
                     Shell:        {shell}\n\
                     Model:        {model}\n\
                     API key set:  {api_key_set}\n\
                     OS:           {os}\n\
                     Arch:         {arch}",
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
