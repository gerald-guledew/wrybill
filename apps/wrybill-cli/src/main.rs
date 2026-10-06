//! The `wrybill` binary. Everything it does is in the `wrybill_cli` library.

use std::process::ExitCode;

fn main() -> ExitCode {
    wrybill_cli::run()
}
