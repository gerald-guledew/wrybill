//! The `wrybill` command line.
//!
//! A thin front door: it reads arguments and prints results, and the work
//! happens in the shared crates (spec 7.2). It's a library as well as a
//! binary, so its pieces can be tested without starting a process.

pub mod logging;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// An adaptive AI agent for your computer.
// `bin_name` keeps the usage line as `wrybill` on every OS, where Windows
// would otherwise show `wrybill.exe`.
#[derive(Debug, Parser)]
#[command(name = "wrybill", bin_name = "wrybill", version)]
struct Cli {}

/// Runs the command line with the arguments this process was started with.
pub fn run() -> ExitCode {
    // `--help` and `--version` are answered here, and anything unknown is an
    // error. Neither creates any file or folder.
    Cli::parse();

    // There are no commands yet, so there's nothing to run. Show the help instead.
    match Cli::command().print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
