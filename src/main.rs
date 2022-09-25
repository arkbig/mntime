//! # mntime
//!
//! The mntime command executes the specified "m" commands "n" times and calculates the mean "μ"(\mu) of usage time and memory.
//!
//! See `mntime --help` for usage.

mod app;
mod cli_args;
mod cmd;
mod stats;
mod terminal;

fn main() {
    let res = app::run();
    proc_exit::exit(res);
}
