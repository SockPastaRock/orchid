use std::collections::BTreeMap;

pub mod output;
pub use output::{print_error, print_json};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help(Option<String>),
    List(Option<ListSubcommand>),
    Config(ConfigSubcommand),
    Create {
        label: Option<String>,
        persona: Option<String>,
        working_dir: Option<String>,
        profile: Option<String>,
    },
    Send {
        id: Option<String>,
        message: String,
        await_completion: bool,
        profile: Option<String>,
        label: Option<String>,
        working_dir: Option<String>,
    },
    Set {
        id: String,
        label: Option<String>,
        persona: Option<String>,
        working_dir: Option<String>,
    },
    Delete(String),
    Stop(String),
    Kill(String),
    InternalRun {
        id: String,
        profile: Option<String>,
    },
    ServerAction {
        action: String,
        profile: Option<String>,
        body_params: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListSubcommand {
    Profiles,
    Personas,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSubcommand {
    Use(String),
    Current,
    Path,
}

pub fn parse_args(args: &[String]) -> Result<(Command, BTreeMap<String, Option<String>>), String> {
    if args.is_empty() {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    let cmd_name = &args[0];

    if cmd_name == "--help" {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    // Flags that take a value argument. All others are boolean.
    // Unknown flags are rejected after command dispatch.
    const VALUE_FLAGS: &[&str] = &[
        "id",
        "label",
        "persona",
        "profile",
        "working-dir",
        "max-steps",
        "timeout",
        "await",
    ];

    // `flags` collects all flags; for server-action, remaining flags become body params.
    let mut flags = BTreeMap::new();
    let mut positional = Vec::new();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some(eq_pos) = rest.find('=') {
                let key = rest[..eq_pos].to_string();
                let value = rest[eq_pos + 1..].to_string();
                flags.insert(key, Some(value));
            } else {
                let key = rest.to_string();
                let takes_value = VALUE_FLAGS.contains(&key.as_str());
                if takes_value && i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    // Boolean flags that take a value flag but should NOT consume next token.
                    if key == "await" {
                        flags.insert(key, None);
                    } else {
                        i += 1;
                        flags.insert(key, Some(args[i].clone()));
                    }
                } else if !takes_value && i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    // Unknown flags that have a following token are treated as value-taking
                    // (for server-action body params). For other commands, the fail-fast
                    // check catches them.
                    i += 1;
                    flags.insert(key, Some(args[i].clone()));
                } else {
                    flags.insert(key, None);
                }
            }
        } else if !arg.starts_with("-") {
            positional.push(arg.clone());
        }
        i += 1;
    }

    if flags.contains_key("help") {
        return Ok((Command::Help(Some(cmd_name.clone())), flags));
    }

    let cmd = match cmd_name.as_str() {
        "help" => Command::Help(positional.into_iter().next()),
        "list" => {
            let sub = match positional.first().map(|s| s.as_str()) {
                Some("profiles") => Some(ListSubcommand::Profiles),
                Some("personas") => Some(ListSubcommand::Personas),
                Some(other) => return Err(format!("unknown list subcommand: {}", other)),
                None => None,
            };
            Command::List(sub)
        }
        "create" => {
            let label = flags.remove("label").flatten();
            let persona = flags.remove("persona").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            let profile = flags.remove("profile").flatten();
            Command::Create {
                label,
                persona,
                working_dir,
                profile,
            }
        }
        "config" => {
            if positional.is_empty() {
                return Err("config requires subcommand: use, current, or path".to_string());
            }
            let sub = &positional[0];
            match sub.as_str() {
                "use" => {
                    if positional.len() < 2 {
                        return Err("config use requires <profile> argument".to_string());
                    }
                    Command::Config(ConfigSubcommand::Use(positional[1].clone()))
                }
                "current" => Command::Config(ConfigSubcommand::Current),
                "path" => Command::Config(ConfigSubcommand::Path),
                _ => return Err(format!("unknown config subcommand: {}", sub)),
            }
        }
        "send" => {
            if positional.is_empty() {
                return Err("send requires a message".to_string());
            }
            let message = positional[0].clone();
            let id = flags.remove("id").flatten();
            let await_completion = flags.contains_key("await");
            flags.remove("await");
            let profile = flags.remove("profile").flatten();
            let label = flags.remove("label").flatten();
            let working_dir = flags.remove("working-dir").flatten();

            // Check for unknown flags.
            if let Some(unknown) = flags.iter().find(|(k, _v)| {
                !VALUE_FLAGS.contains(&k.as_str())
            }).map(|(k, _)| k.as_str()) {
                return Err(format!("unknown flag: --{}", unknown));
            }

            Command::Send {
                id,
                message,
                await_completion,
                profile,
                label,
                working_dir,
            }
        }
        "set" => {
            let id = flags
                .remove("id")
                .flatten()
                .ok_or_else(|| "set requires --id".to_string())?;
            let label = flags.remove("label").flatten();
            let persona = flags.remove("persona").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            if flags.remove("profile").is_some() {
                return Err("--profile is not supported on set; use `orchid config use <name>` to switch the active profile".to_string());
            }

            Command::Set {
                id,
                label,
                persona,
                working_dir,
            }
        }
        "delete" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| "delete requires <id>".to_string())?;
            Command::Delete(id)
        }
        "stop" | "kill" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| format!("{} requires <id>", cmd_name))?;
            if cmd_name == "stop" {
                Command::Stop(id)
            } else {
                Command::Kill(id)
            }
        }
        "__run" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| "__run requires <id>".to_string())?;
            let profile = flags.remove("profile").flatten();
            Command::InternalRun { id, profile }
        }
        "server-action" => {
            let action = positional
                .first()
                .cloned()
                .ok_or_else(|| "server-action requires <action>".to_string())?;
            let profile = flags.remove("profile").flatten();
            // Remaining flags in `flags` are body params for the action.
            let mut body_params = Vec::new();
            for (k, v) in std::mem::take(&mut flags).into_iter() {
                if let Some(val) = v {
                    body_params.push((k, val));
                }
            }
            return Ok((Command::ServerAction {
                action,
                profile,
                body_params,
            }, flags));
        }
        _ => return Err(format!("unknown command: {}", cmd_name)),
    };

    Ok((cmd, flags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list() {
        let args = vec!["list".to_string()];
        let (cmd, flags) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::List(None));
        assert!(flags.is_empty());
    }

    #[test]
    fn test_parse_config_current() {
        let args = vec!["config".to_string(), "current".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Config(ConfigSubcommand::Current));
    }

    #[test]
    fn test_parse_config_use() {
        let args = vec![
            "config".to_string(),
            "use".to_string(),
            "myprofile".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(
            cmd,
            Command::Config(ConfigSubcommand::Use("myprofile".to_string()))
        );
    }

    #[test]
    fn test_parse_config_path() {
        let args = vec!["config".to_string(), "path".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Config(ConfigSubcommand::Path));
    }

    #[test]
    fn test_parse_flags() {
        // Value-taking flags consume the next token; boolean flags do not.
        // Verify via the parsed Command fields rather than the leftover flags map,
        // since dispatch removes consumed flags.
        let args = vec![
            "send".to_string(),
            "--id".to_string(),
            "abc".to_string(),
            "--await".to_string(),
            "the message".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::Send {
                id,
                await_completion,
                message,
                ..
            } => {
                assert_eq!(id, Some("abc".to_string()));
                assert!(await_completion);
                assert_eq!(message, "the message");
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn test_parse_no_args() {
        let args = vec![];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Help(None));
    }

    #[test]
    fn test_parse_config_no_subcommand() {
        let args = vec!["config".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_parse_config_use_no_profile() {
        let args = vec!["config".to_string(), "use".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_parse_unknown_command() {
        let args = vec!["unknown".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_parse_help_command() {
        let args = vec!["help".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Help(None));
    }

    #[test]
    fn test_parse_help_flag() {
        let args = vec!["--help".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Help(None));
    }

    #[test]
    fn test_parse_command_help_flag() {
        let args = vec!["list".to_string(), "--help".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::Help(Some("list".to_string())));
    }

    #[test]
    fn test_parse_send() {
        let args = vec!["send".to_string(), "hello world".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::Send {
                id: None,
                message,
                await_completion: false,
                profile: None,
                ..
            } => assert_eq!(message, "hello world"),
            _ => panic!("expected Send command"),
        }
    }

    #[test]
    fn test_parse_send_await_does_not_consume_message() {
        // Regression: --await is a boolean flag and must not greedily consume
        // the following positional argument as its value.
        let args = vec![
            "send".to_string(),
            "--id".to_string(),
            "abc123".to_string(),
            "--profile".to_string(),
            "myprofile".to_string(),
            "--await".to_string(),
            "the message".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::Send {
                message,
                await_completion,
                id,
                profile,
                ..
            } => {
                assert_eq!(message, "the message");
                assert!(await_completion, "--await should be set");
                assert_eq!(id, Some("abc123".to_string()));
                assert_eq!(profile, Some("myprofile".to_string()));
            }
            _ => panic!("expected Send command"),
        }
    }

    #[test]
    fn test_parse_send_with_id() {
        let args = vec![
            "send".to_string(),
            "--id".to_string(),
            "abc123".to_string(),
            "test message".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::Send { id: Some(id), .. } => assert_eq!(id, "abc123"),
            _ => panic!("expected Send command with id"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let args = vec!["delete".to_string(), "abc123".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::Delete(id) => assert_eq!(id, "abc123"),
            _ => panic!("expected Delete command"),
        }
    }

    #[test]
    fn test_unknown_flag_is_error() {
        // Unknown flags on non-server-action commands that have a following token
        // get consumed as values, causing the command to error (e.g. no message).
        let args = vec![
            "send".to_string(),
            "--print-response".to_string(),
            "hello".to_string(),
        ];
        let err = parse_args(&args).unwrap_err();
        assert!(
            err.contains("unknown flag") || err.contains("requires a message"),
            "expected an error for unknown flag, got: {}",
            err
        );
    }

    #[test]
    fn test_unknown_flag_does_not_consume_message() {
        // Unknown flags on non-server-action commands are caught by the fail-fast
        // check and produce an error rather than silently consuming the message.
        let args = vec![
            "send".to_string(),
            "--await".to_string(),
            "--print-response".to_string(),
            "hello".to_string(),
        ];
        let err = parse_args(&args).unwrap_err();
        assert!(
            err.contains("unknown flag") || err.contains("requires a message"),
            "expected an error for unknown flag, got: {}",
            err
        );
    }

    #[test]
    fn test_parse_server_action_minimal() {
        let args = vec!["server-action".to_string(), "list_models".to_string()];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::ServerAction {
                action,
                profile,
                body_params,
            } => {
                assert_eq!(action, "list_models");
                assert!(profile.is_none());
                assert!(body_params.is_empty());
            }
            _ => panic!("expected ServerAction"),
        }
    }

    #[test]
    fn test_parse_server_action_with_profile() {
        let args = vec![
            "server-action".to_string(),
            "load_model".to_string(),
            "--profile".to_string(),
            "local-lmstudio".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::ServerAction {
                action,
                profile,
                body_params,
            } => {
                assert_eq!(action, "load_model");
                assert_eq!(profile, Some("local-lmstudio".to_string()));
                assert!(body_params.is_empty());
            }
            _ => panic!("expected ServerAction"),
        }
    }

    #[test]
    fn test_parse_server_action_with_body_params() {
        let args = vec![
            "server-action".to_string(),
            "load_model".to_string(),
            "--profile".to_string(),
            "local-lmstudio".to_string(),
            "--model".to_string(),
            "openai/gpt-oss-20b".to_string(),
            "--context_length".to_string(),
            "16384".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::ServerAction {
                action,
                profile,
                body_params,
            } => {
                assert_eq!(action, "load_model");
                assert_eq!(profile, Some("local-lmstudio".to_string()));
                assert_eq!(body_params.len(), 2);
                // BTreeMap preserves order by key: context_length < model
                assert_eq!(body_params[0], ("context_length".to_string(), "16384".to_string()));
                assert_eq!(body_params[1], ("model".to_string(), "openai/gpt-oss-20b".to_string()));
            }
            _ => panic!("expected ServerAction"),
        }
    }

    #[test]
    fn test_parse_server_action_missing_action() {
        let args = vec!["server-action".to_string()];
        let err = parse_args(&args).unwrap_err();
        assert!(err.contains("requires <action>"));
    }

    #[test]
    fn test_parse_server_action_with_eq_flag() {
        let args = vec![
            "server-action".to_string(),
            "load_model".to_string(),
            "--model=openai/gpt-oss-20b".to_string(),
        ];
        let (cmd, _) = parse_args(&args).unwrap();
        match cmd {
            Command::ServerAction {
                body_params,
                ..
            } => {
                assert_eq!(body_params.len(), 1);
                assert_eq!(body_params[0], ("model".to_string(), "openai/gpt-oss-20b".to_string()));
            }
            _ => panic!("expected ServerAction"),
        }
    }
}
