pub fn parse() -> CliArgs {
    CliArgs::parse()
}

use clap::Parser;
/// Command Line Arguments
#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, setting = clap::builder::AppSettings::TrailingVarArg | clap::builder::AppSettings::DeriveDisplayOrder)]
pub struct CliArgs {
    /// Perform exactly NUM runs for each command.
    #[clap(short, long, value_parser, value_name = "NUM", default_value_t = 5)]
    runs: u16,

    /// Time command used.
    #[clap(
        short = 'T',
        long,
        value_parser,
        value_name = "COMMAND",
        default_value = "gtime"
    )]
    time_command: String,

    /// Arguments of the time command used.
    ///
    /// Quoting if flag is included or there are multiple args.
    #[clap(short, long, value_parser, value_name = "ARGS", default_value = "")]
    time_args: String,

    /// The commands to benchmark.
    ///
    /// If multiple commands are specified, each is executed and compared.
    /// One command is specified with "--" delimiters (recommended) or quotation.
    /// However, in the case of command-only quotation marks,
    /// the subsequent ones are considered to be the arguments of the command.
    ///
    /// e.g.) mntime command1 --flag arg -- command2 -- 'command3 -f -- args'
    #[clap(value_parser)]
    commands: Vec<String>,
}
