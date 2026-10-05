//! The `wrybill` command line.
//!
//! A thin front door: it reads arguments and prints results, and the work
//! happens in the shared crates (spec 7.2).

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// An adaptive AI agent for your computer.
// `bin_name` keeps the usage line as `wrybill` on every OS, where Windows
// would otherwise show `wrybill.exe`.
#[derive(Debug, Parser)]
#[command(name = "wrybill", bin_name = "wrybill", version)]
struct Cli {}

fn main() -> ExitCode {
    // `--help` and `--version` are answered here, and anything unknown is an error.
    Cli::parse();

    // There are no commands yet, so there's nothing to run. Show the help instead.
    match Cli::command().print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
