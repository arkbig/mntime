//! # mntime
//!
//! The mntime command executes the specified "m" commands "n" times and calculates the mean "μ"(\mu) of usage time and memory.
//!
//! See `mntime --help` for usage.

mod app;

fn main() {
    let res = app::run();
    proc_exit::exit(res);
}
