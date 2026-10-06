//! The `wrybill` command line.
//!
//! A thin front door: it reads arguments and prints results, and the work
//! happens in the shared crates (spec 7.2). It's a library as well as a
//! binary, so its pieces can be tested without starting a process.

pub mod doctor;
pub mod keys;
pub mod logging;

use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use wrybill_config::{Keychain, Paths};

use crate::logging::Logging;

/// The most that's read when a key is piped in. Far more than any key needs.
const MOST_PIPED_BYTES: u64 = 64 * 1024;

/// An adaptive AI agent for your computer.
// `bin_name` keeps the usage line as `wrybill` on every OS, where Windows
// would otherwise show `wrybill.exe`.
//
// None of these types derive `Debug`: what's typed on the command line is
// never printed back.
#[derive(Parser)]
#[command(name = "wrybill", bin_name = "wrybill", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the system profile and check the setup
    ///
    /// The output names no computer, user or network address, and never
    /// shows a key, so it's safe to paste into a public issue.
    Doctor,

    /// Save the API keys Wrybill uses
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Keys {
        #[command(subcommand)]
        command: KeysCommand,
    },
}

#[derive(Subcommand)]
enum KeysCommand {
    /// Store an API key in the OS keychain
    ///
    /// The key is typed at a hidden prompt, or piped in. It's never given on
    /// the command line, so it stays out of your shell history.
    Set {
        /// Who the key is for, such as anthropic, openai, gemini or brave
        provider: String,

        // Anything typed after the name. A key doesn't belong on the command
        // line, so this is counted, refused and never read.
        #[arg(
            hide = true,
            num_args = 0..,
            allow_hyphen_values = true,
            trailing_var_arg = true
        )]
        extra: Vec<String>,
    },
}

/// Runs the command line with the arguments this process was started with.
pub fn run() -> ExitCode {
    // `--help` and `--version` are answered here, and anything unknown is an
    // error. Neither creates any file or folder.
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        // With no command there's nothing to run, so show the help. This
        // creates nothing either.
        return match Cli::command().print_help() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    };

    let status = match command {
        Command::Doctor => {
            start_log(&mut io::stderr());
            doctor::run(&Keychain, &mut io::stdout())
        }
        Command::Keys {
            command: KeysCommand::Set { provider, extra },
        } => {
            // Only how many there are is passed on. What they say never is.
            let extra_arguments = extra.len();
            drop(extra);

            start_log(&mut io::stderr());
            keys::set(
                &provider,
                extra_arguments,
                &mut read_key,
                &Keychain,
                &mut io::stdout(),
                &mut io::stderr(),
            )
        }
    };
    ExitCode::from(status)
}

/// Starts Wrybill's own log for this run. If there won't be one, it says so
/// and the command carries on.
fn start_log(err: &mut dyn Write) {
    let wanted = std::env::var(logging::LEVEL_VARIABLE).ok();
    let level = logging::level_from(wanted.as_deref()).unwrap_or_else(|_| {
        let _ = writeln!(
            err,
            "Note: {} isn't one of error, warn, info, debug or trace, so the log is at info.",
            logging::LEVEL_VARIABLE
        );
        logging::DEFAULT_LEVEL
    });

    let logging = match Paths::from_env() {
        Ok(paths) => logging::start(&paths, level),
        Err(error) => Logging::Off {
            reason: error.to_string(),
        },
    };
    if let Logging::Off { reason } = logging {
        let _ = writeln!(
            err,
            "Note: Wrybill isn't keeping a log of this run. Reason: {reason}"
        );
    }
}

/// Asks for a key: at a hidden prompt when someone is at the terminal, and
/// from standard input when the key is piped in.
fn read_key(prompt: &str) -> io::Result<String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        rpassword::prompt_password(prompt)
    } else {
        let mut piped = String::new();
        stdin
            .lock()
            .take(MOST_PIPED_BYTES)
            .read_to_string(&mut piped)?;
        Ok(piped)
    }
}
