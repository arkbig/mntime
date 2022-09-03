use clap::{AppSettings, Parser};

/// Command Line Args
// TODO: I want to embed package.name in the about document.
// TODO: I want to change the value name of an option. e.g. -w, --warmup <NUM>
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, setting = AppSettings::TrailingVarArg)]
struct CliArgs {
    /// Perform exactly NUM runs for each command.
    #[clap(short, long, value_parser, default_value_t = 5)]
    runs: u16,

    /// Time command used.
    #[clap(short='T', long, value_parser, default_value = "gtime")]
    time_command: String,

    /// Arguments of the time command used.
    #[clap(short, long, value_parser, default_value = "")]
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

fn main() {
    let args = CliArgs::parse();

    for c in args.commands {
        println!("{}", c)
    }
}
