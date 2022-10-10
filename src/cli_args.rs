pub fn parse() -> CliArgs {
    CliArgs::parse()
}

use clap::Parser;
/// Command Line Arguments
#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, setting = clap::builder::AppSettings::TrailingVarArg | clap::builder::AppSettings::DeriveDisplayOrder)]
pub struct CliArgs {
    /// Perform NUM runs for each command.
    #[clap(short, long, value_parser, value_name = "NUM", default_value_t = 10)]
    pub runs: u16,

    /// Loop NUM times with one measurement run for each command.
    ///
    /// That is, each command is executed "runs" × "loops" times.
    ///
    /// This is used when a single run is very fast and does not meet the resolution of the time command.
    /// (e.g., less than 10 ms).
    ///
    /// Statistics are calculated in a NUM loop, but are displayed as a value per one run.
    /// Specifically, time-related items, such as "User time", is divided by NUM and displayed,
    /// and memory-related items, such as "Maximum resident set size", are displayed as they are.
    /// Divided values are indicated by "/NUM" in the item name.
    ///
    /// The loop uses a "for" statement in "sh", so the extra processing is measured.
    /// But if loops is 1, it is executed directly without "for" statement.
    #[clap(long, value_parser, value_name = "NUM", default_value_t = 1)]
    pub loops: u16,

    /// Set the shell to use for executing benchmarked commands.
    ///
    /// This is executed as `sh -c time command1`.
    /// If execution confirmation is not obtained, also try `/usr/bin/env bash`.
    ///
    /// e.g.) sh, /opt/homebrew/bin/zsh
    #[clap(short = 'S', long, value_name = "COMMAND", default_value = "sh")]
    pub shell: String,

    /// Set the shell args to use for executing benchmarked commands.
    ///
    /// This would be specified when executing in a POSIX incompatible shell.
    #[clap(long, value_name = "ARG", default_value = "-c")]
    pub shell_arg: String,

    /// Use shell built-in time.
    #[clap(long)]
    pub use_builtin: bool,

    /// Change shell built-in time command.
    #[clap(long, value_name = "COMMAND", default_value = "time")]
    pub builtin: String,

    /// Use BSD time.
    ///
    /// The default is to try to run BSD and GNU alternately.
    /// If neither of those is available, use built-in.
    #[clap(long)]
    pub use_bsd: bool,

    /// Change BSD time command.
    #[clap(long, value_name = "COMMAND", default_value = "/usr/bin/env time -l")]
    pub bsd: String,

    /// Use GNU time.
    ///
    /// The default is to try to run BSD and GNU alternately.
    /// If neither of those is available, use built-in.
    #[clap(long)]
    pub use_gnu: bool,

    /// Change GNU time command.
    ///
    /// If execution confirmation is not obtained, also try `/usr/bin/env time -v`.
    #[clap(long, value_name = "COMMAND", default_value = "gtime -v")]
    pub gnu: String,

    /// The commands to benchmark.
    ///
    /// If multiple commands are specified, each is executed and compared.
    /// One command is specified with "--" delimiters (recommended) or quotation.
    /// However, in the case of command-only quotation marks,
    /// the subsequent ones are considered to be the arguments of the command.
    ///
    /// e.g.) mntime command1 --flag arg -- command2 -- 'command3 -f -- args' command4 -o "output files"
    #[clap(value_parser)]
    commands: Vec<String>,
}

impl CliArgs {
    pub fn normalized_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();
        let delimiters = "--";
        let mut one_command_and_args = Vec::new();
        for s in &self.commands {
            let one = s.clone();
            if one_command_and_args.is_empty() {
                if one == delimiters {
                    // nop
                } else if one.contains(' ') {
                    // These are one quoted command.
                    commands.push(one);
                } else {
                    // This is a command.
                    one_command_and_args.push(one);
                }
            } else if one == delimiters {
                // One command determined.
                if !one_command_and_args.is_empty() {
                    commands.push(one_command_and_args.join(" "));
                    one_command_and_args.clear();
                }
            } else {
                one_command_and_args.push(to_quoted(one));
            }
        }
        if !one_command_and_args.is_empty() {
            commands.push(one_command_and_args.join(" "));
        }
        commands
    }
}

fn is_quoted(str: &str) -> bool {
    str.starts_with('"') && str.ends_with('"') || str.starts_with('\'') && str.ends_with('\'')
}

fn to_quoted(str: String) -> String {
    if is_quoted(&str) || str.starts_with('-') {
        return str;
    }
    format!("'{}'", str.replace('\'', "\\'"))
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn cli_args_normalized_commands() {
        // only command
        let cli_args = CliArgs::parse_from(vec!["mntime", "cmd1"]);
        let commands = cli_args.normalized_commands();
        assert_eq!(commands, vec!["cmd1"]);

        // one command and arg pair
        let cli_args = CliArgs::parse_from(vec!["mntime", "cmd1", "arg1"]);
        let commands = cli_args.normalized_commands();
        assert_eq!(commands, vec!["cmd1 'arg1'"]);

        // two commands
        let cli_args =
            CliArgs::parse_from(vec!["mntime", "cmd1", "arg1", "--", "cmd2", "arg1", "arg2"]);
        let commands = cli_args.normalized_commands();
        assert_eq!(commands, vec!["cmd1 'arg1'", "cmd2 'arg1' 'arg2'"]);

        // quoted separator
        let cli_args = CliArgs::parse_from(vec!["mntime", "cmd1 arg1", "cmd2 arg1 arg2"]);
        let commands = cli_args.normalized_commands();
        assert_eq!(commands, vec!["cmd1 arg1", "cmd2 arg1 arg2"]);

        // quoted args
        let cli_args = CliArgs::parse_from(vec![
            "mntime",
            "cmd1",
            "arg1",
            "--",
            "cmd2",
            "\"arg1 arg2\"",
        ]);
        let commands = cli_args.normalized_commands();
        assert_eq!(commands, vec!["cmd1 'arg1'", "cmd2 \"arg1 arg2\""]);

        // combination
        let cli_args = CliArgs::parse_from(vec![
            "mntime",
            "command1",
            "--flag",
            "arg",
            "--",
            "command2",
            "--",
            "command3 -f -- args",
            "command4",
            "-o",
            "output files",
        ]);
        let commands = cli_args.normalized_commands();
        assert_eq!(
            commands,
            vec![
                "command1 --flag 'arg'",
                "command2",
                "command3 -f -- args",
                "command4 -o 'output files'"
            ]
        );
    }
}
