use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::cli_args::Cli;

pub fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let bin_name = env!("CARGO_PKG_NAME");
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
}

pub fn print_help_json() {
    let json_help = build_command_json(&Cli::command());
    println!(
        "{}",
        serde_json::to_string_pretty(&json_help).expect("Failed to serialize help to JSON")
    );
}

fn build_command_json(cmd: &clap::Command) -> serde_json::Value {
    let mut subcommands = Vec::new();
    for subcommand in cmd.get_subcommands() {
        subcommands.push(serde_json::json!({
            "name": subcommand.get_name().to_string(),
            "about": subcommand.get_about().map(|s| s.to_string()),
            "args": build_args_json(subcommand),
        }));
    }

    serde_json::json!({
        "name": cmd.get_name().to_string(),
        "about": cmd.get_about().map(|s| s.to_string()),
        "version": cmd.get_version().map(|v| v.to_string()),
        "subcommands": subcommands,
        "global_args": build_args_json(cmd),
    })
}

fn build_args_json(cmd: &clap::Command) -> serde_json::Value {
    let mut args = Vec::new();
    for arg in cmd.get_arguments() {
        let short = arg.get_short().map(|c| format!("-{}", c));
        let long = arg.get_long().map(|s| format!("--{}", s));
        let default_value = arg
            .get_default_values()
            .first()
            .and_then(|v| v.to_str())
            .map(|s| s.to_string());
        let possible_values_vec = arg.get_possible_values();
        let possible_values: Option<Vec<&str>> = if possible_values_vec.is_empty() {
            None
        } else {
            Some(possible_values_vec.iter().map(|pv| pv.get_name()).collect())
        };

        args.push(serde_json::json!({
            "id": arg.get_id().to_string(),
            "short": short,
            "long": long,
            "help": arg.get_help().map(|s| s.to_string()),
            "required": arg.is_required_set(),
            "default_value": default_value,
            "possible_values": possible_values,
        }));
    }
    serde_json::json!(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    #[test]
    fn build_args_json_describes_flags() {
        let cmd = Command::new("demo").arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .help("Mode to use")
                .default_value("fast")
                .value_parser(["fast", "slow"]),
        );
        let json = build_args_json(&cmd);
        let mode = json
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "mode")
            .unwrap();
        assert_eq!(mode["short"], "-m");
        assert_eq!(mode["long"], "--mode");
        assert_eq!(mode["help"], "Mode to use");
        assert_eq!(mode["required"], false);
        assert_eq!(mode["default_value"], "fast");
        assert_eq!(mode["possible_values"], serde_json::json!(["fast", "slow"]));
    }

    #[test]
    fn build_args_json_uses_null_for_missing_values() {
        let cmd = Command::new("demo").arg(Arg::new("path").required(true));
        let json = build_args_json(&cmd);
        let path = &json.as_array().unwrap()[0];
        assert_eq!(path["short"], serde_json::Value::Null);
        assert_eq!(path["long"], serde_json::Value::Null);
        assert_eq!(path["default_value"], serde_json::Value::Null);
        assert_eq!(path["possible_values"], serde_json::Value::Null);
        assert_eq!(path["required"], true);
    }

    #[test]
    fn build_command_json_lists_cli_subcommands() {
        let json = build_command_json(&Cli::command());
        let names: Vec<&str> = json["subcommands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        for expected in ["install", "list", "select", "fix", "help-json"] {
            assert!(names.contains(&expected), "missing subcommand {expected}");
        }
        assert!(json["global_args"].is_array());
    }
}
