pub fn help() -> Result<serde_json::Value, String> {
    let text = r#"orchid - conversation management CLI

USAGE:
  orchid <COMMAND> [OPTIONS]

COMMANDS:
  list                List all conversations
  config              Manage profiles (use, current, path)
  create              Create a new conversation without sending a message
  send                Send message to conversation (requires --id or stores in current)
  set                 Update conversation settings
  delete              Delete conversation by ID
  help                Display this help message

OPTIONS:
  --help              Show help for a command
  --id <ID>           Conversation ID
  --await             Wait for completion after send
  --profile <NAME>    Use specific profile
  --label <TEXT>      Set conversation label
  --persona <TEXT>    Set persona/system prompt
  --working-dir <PATH> Set working directory

EXAMPLES:
  orchid help                              Show this help
  orchid list                              List conversations
  orchid create --label "my-task" --working-dir /path/to/project
  orchid send "hello" --id abc123          Send message
  orchid config current                    Show current profile
  orchid set --id abc123 --label "work"    Update label

For command-specific help: orchid <COMMAND> --help"#;

    println!("{}", text);
    Ok(serde_json::Value::Null)
}

pub fn help_command(cmd: &str) -> Result<serde_json::Value, String> {
    let text = match cmd {
        "list" => "orchid list - List all conversations\n\nUsage: orchid list\n\nShows all stored conversations.",
        "config" => "orchid config - Manage profiles\n\nUsage: orchid config <SUBCOMMAND>\n\nSubcommands:\n  use <NAME>   Switch to profile\n  current      Show current profile\n  path         Show config path",
        "create" => "orchid create - Create a new conversation\n\nUsage: orchid create [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --persona <TEXT>     Set system prompt\n  --working-dir <PATH> Set working directory\n  --profile <NAME>     Use specific profile",
        "send" => "orchid send - Send message to conversation\n\nUsage: orchid send <MESSAGE> [OPTIONS]\n\nOptions:\n  --id <ID>      Target conversation (required if no current)\n  --await        Wait for response\n  --profile <NAME> Use specific profile",
        "set" => "orchid set - Update conversation settings\n\nUsage: orchid set --id <ID> [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --persona <TEXT>     Set system prompt\n  --working-dir <PATH> Set working directory\n  --profile <NAME>     Use specific profile",
        "delete" => "orchid delete - Archive conversation\n\nUsage: orchid delete <ID>\n\nMoves the conversation to ~/.config/orchid/conversations/.archive/<id>.\nRemoved from orchid list. Reversible: move the directory back to restore.",
        "help" => "orchid help - Display help\n\nUsage: orchid help\n       orchid --help\n       orchid <COMMAND> --help\n\nShow usage information.",
        _ => return Err(format!("unknown command: {}", cmd)),
    };

    println!("{}", text);
    Ok(serde_json::Value::Null)
}
