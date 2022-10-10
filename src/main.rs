//! # mntime
//!
//! The mntime command executes the specified "m" commands "n" times and calculates the mean of usage time and memory.
//!
//! See `mntime --help` for usage.
//! 
//! In source code, [cli_args::CliArgs].
//! 
//! Copyright © ArkBig

mod app;
mod cli_args;
mod cmd;
mod stats;
mod terminal;

fn main() {
    let res = app::run();
    proc_exit::exit(res);
}
