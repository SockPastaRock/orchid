use orchid::cli::{output, parse_args, Command, ConfigSubcommand, ListSubcommand};
use orchid::cmd;
use orchid::JsonError;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let args_slice = if args.len() > 1 { &args[1..] } else { &[] };

    let (cmd, _flags) = match parse_args(args_slice) {
        Ok((c, f)) => (c, f),
        Err(e) => {
            let err = JsonError::new("invalid_args", &e);
            let _ = output::print_error(&err);
            process::exit(1);
        }
    };

    let result = match cmd {
        Command::Help(None) => cmd::help(),
        Command::Help(Some(ref cmd_name)) => cmd::help_command(cmd_name),
        Command::List(None) => cmd::list(),
        Command::List(Some(ListSubcommand::Profiles)) => cmd::list_profiles(),
        Command::List(Some(ListSubcommand::Personas)) => cmd::list_personas(),
        Command::Config(ConfigSubcommand::Current) => cmd::config_current(),
        Command::Config(ConfigSubcommand::Path) => cmd::config_path(),
        Command::Config(ConfigSubcommand::Use(profile)) => cmd::config_use(&profile),
        Command::Create {
            label,
            persona,
            working_dir,
            profile,
        } => cmd::create(label, persona, working_dir, profile),
        Command::Send {
            id,
            message,
            await_completion,
            profile,
            label,
            working_dir,
        } => cmd::send(id, message, await_completion, profile, label, working_dir),
        Command::Set {
            id,
            label,
            persona,
            working_dir,
        } => cmd::set(id, label, persona, working_dir),
        Command::Delete(id) => cmd::delete(id),
        Command::Stop(id) => cmd::stop(id),
        Command::Kill(id) => cmd::stop(id),
        Command::InternalRun { id, profile } => match cmd::internal_run(&id, &profile) {
            Ok(()) => Ok(serde_json::json!({"status": "ok"})),
            Err(e) => Err(e),
        },
    };

    match result {
        Ok(json) => {
            if json.is_null() {
                return;
            }
            if let Err(e) = output::print_json(&json) {
                let err = JsonError::new("output_error", &e);
                let _ = output::print_error(&err);
                process::exit(1);
            }
        }
        Err(e) => {
            let err = JsonError::new("command_error", &e);
            let _ = output::print_error(&err);
            process::exit(1);
        }
    }
}
