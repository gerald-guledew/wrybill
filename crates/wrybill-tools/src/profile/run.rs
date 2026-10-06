//! Runs a program to ask it one question, such as its version.
//!
//! These runs are a fixed, read-only list that Wrybill itself decided on.
//! They aren't actions a brain proposed, so they don't go through the
//! Guardian (docs/DECISIONS.md). Each one still gets:
//!
//! - an argument list, never a shell;
//! - a time limit, after which the program is stopped;
//! - a neutral working folder, the top of the disk, so nothing in the folder
//!   the user is in, or in any project, can change the answer;
//! - no input, and a cap on how much output is kept;
//! - the user's own environment, so version managers keep working.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The most output that's kept from each of a program's two streams.
const MOST_OUTPUT: usize = 16 * 1024;

/// How often a running program is checked on.
const CHECK_EVERY: Duration = Duration::from_millis(10);

/// How long to wait for the last of a program's output once it has ended.
/// Something it started may still hold the pipe open, and isn't waited for.
const OUTPUT_GRACE: Duration = Duration::from_millis(250);

/// A program to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Job {
    /// The program's full path.
    pub(super) program: PathBuf,
    /// What to pass it.
    pub(super) args: &'static [&'static str],
    /// How long it may take.
    pub(super) limit: Duration,
}

/// What a program did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ran {
    /// Whether it ended by itself and reported success.
    pub(super) succeeded: bool,
    /// What it printed, as far as that's kept.
    pub(super) stdout: String,
    /// What it printed as errors, as far as that's kept.
    pub(super) stderr: String,
}

/// Why there's nothing to show for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunError {
    /// The program couldn't be started.
    NotStarted,
    /// It was still running when its time was up, so it was stopped.
    TimedOut,
}

/// Runs every job, at most `at_once` of them at a time. The results come
/// back in the order of the jobs.
pub(super) fn run_all(jobs: &[Job], at_once: usize) -> Vec<Result<Ran, RunError>> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<Ran, RunError>>>> =
        Mutex::new(jobs.iter().map(|_| None).collect());

    thread::scope(|scope| {
        for _ in 0..at_once.clamp(1, jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else {
                        break;
                    };
                    let result = run(&job.program, job.args, job.limit);
                    if let Ok(mut results) = results.lock()
                        && let Some(slot) = results.get_mut(index)
                    {
                        *slot = Some(result);
                    }
                }
            });
        }
    });

    results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .into_iter()
        .map(|result| result.unwrap_or(Err(RunError::NotStarted)))
        .collect()
}

/// Runs one program and waits for it, for no longer than `limit`.
pub(super) fn run(program: &Path, args: &[&str], limit: Duration) -> Result<Ran, RunError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(neutral_folder())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        // `CREATE_NO_WINDOW`: a console program gets no window of its own,
        // so nothing flashes up when Wrybill itself has no console.
        command.creation_flags(0x0800_0000);
    }

    let mut child = command.spawn().map_err(|_| RunError::NotStarted)?;
    let stdout = child.stdout.take().map(Kept::from);
    let stderr = child.stderr.take().map(Kept::from);

    let give_up_at = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < give_up_at => thread::sleep(CHECK_EVERY),
            Ok(None) | Err(_) => break None,
        }
    };
    let Some(status) = status else {
        // Stop it, and wait so that it doesn't linger as a dead process.
        let _ = child.kill();
        let _ = child.wait();
        return Err(RunError::TimedOut);
    };

    Ok(Ran {
        succeeded: status.success(),
        stdout: stdout.map(Kept::text).unwrap_or_default(),
        stderr: stderr.map(Kept::text).unwrap_or_default(),
    })
}

/// A folder where nothing anyone is working on can affect a program: the top
/// of the disk the OS is on. That's `/`, or on Windows a folder such as
/// `C:\`.
///
/// No folder is above it, so a version manager that looks upwards for a
/// project's settings finds none, and only an administrator can put a file
/// there.
///
/// It isn't the folder for temporary files. That one follows `TMPDIR`, which
/// can point into a project, and on Linux it's usually `/tmp`, where every
/// user of the computer can leave files.
fn neutral_folder() -> PathBuf {
    #[cfg(windows)]
    {
        // `SystemRoot` is where Windows itself is installed, such as
        // `C:\Windows`.
        if let Some(top) = top_of_the_disk(std::env::var_os("SystemRoot").as_deref()) {
            return top;
        }
    }
    // `/`. On Windows this is the top of the current drive, for a Windows
    // that doesn't say where it's installed.
    PathBuf::from(std::path::MAIN_SEPARATOR_STR)
}

/// The top of the disk that `folder` is on. `None` unless `folder` is a full
/// path on a disk that's there.
#[cfg(any(windows, test))]
fn top_of_the_disk(folder: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let top = Path::new(folder?).ancestors().last()?;
    (top.is_absolute() && top.is_dir()).then(|| top.to_path_buf())
}

/// The start of what a program wrote to one of its streams, read on a thread
/// of its own so the program is never left waiting for a reader.
struct Kept {
    bytes: Arc<Mutex<Vec<u8>>>,
    finished: Receiver<()>,
}

impl Kept {
    fn from(mut stream: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (finish, finished) = mpsc::channel();

        let kept = Arc::clone(&bytes);
        thread::spawn(move || {
            let mut chunk = [0_u8; 1024];
            loop {
                let read = match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                let Ok(mut kept) = kept.lock() else {
                    break;
                };
                let room = MOST_OUTPUT.saturating_sub(kept.len());
                kept.extend_from_slice(chunk.get(..read.min(room)).unwrap_or_default());
                if kept.len() >= MOST_OUTPUT {
                    // Enough. Closing the stream tells the program to stop
                    // writing.
                    break;
                }
            }
            let _ = finish.send(());
        });

        Self { bytes, finished }
    }

    /// What was kept, once the program has ended.
    fn text(self) -> String {
        let _ = self.finished.recv_timeout(OUTPUT_GRACE);
        match self.bytes.lock() {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::{Job, MOST_OUTPUT, RunError, neutral_folder, run, run_all, top_of_the_disk};

    /// The name of the helper below, as the test harness knows it.
    const HELPER: &str = "profile::run::tests::helper";

    /// Not a test of anything. It's the program that the tests below run:
    /// this test binary, asked to run only this function. It does what the
    /// `helper-does=` argument says, and nothing when there isn't one.
    #[test]
    fn helper() {
        let wanted =
            std::env::args().find_map(|arg| arg.strip_prefix("helper-does=").map(str::to_owned));
        match wanted.as_deref() {
            Some("print") => {
                println!("said on standard output");
                eprintln!("said on standard error");
            }
            Some("fail") => std::process::exit(3),
            Some("sleep") => std::thread::sleep(Duration::from_secs(120)),
            Some("flood") => {
                let line = "x".repeat(1023);
                for _ in 0..4096 {
                    println!("{line}");
                }
            }
            Some("where") => {
                let here = std::env::current_dir().expect("a current folder");
                println!("in:{}", here.display());
            }
            _ => {}
        }
    }

    fn this_test_binary() -> PathBuf {
        std::env::current_exe().expect("the path of the test binary")
    }

    fn helper_args(does: &'static str) -> &'static [&'static str] {
        match does {
            "print" => &["--exact", HELPER, "--nocapture", "helper-does=print"],
            "fail" => &["--exact", HELPER, "--nocapture", "helper-does=fail"],
            "sleep" => &["--exact", HELPER, "--nocapture", "helper-does=sleep"],
            "flood" => &["--exact", HELPER, "--nocapture", "helper-does=flood"],
            "where" => &["--exact", HELPER, "--nocapture", "helper-does=where"],
            _ => &["--exact", HELPER, "--nocapture"],
        }
    }

    fn run_helper(does: &'static str, limit: Duration) -> Result<super::Ran, RunError> {
        run(&this_test_binary(), helper_args(does), limit)
    }

    const PLENTY: Duration = Duration::from_secs(60);

    #[test]
    fn what_a_program_prints_comes_back_from_both_streams() {
        let ran = run_helper("print", PLENTY).expect("the helper should run");

        assert!(ran.succeeded);
        assert!(ran.stdout.contains("said on standard output"), "{ran:?}");
        assert!(ran.stderr.contains("said on standard error"), "{ran:?}");
    }

    #[test]
    fn a_program_that_reports_failure_is_not_a_success() {
        let ran = run_helper("fail", PLENTY).expect("the helper should run");

        assert!(!ran.succeeded);
    }

    #[test]
    fn a_program_that_takes_too_long_is_stopped() {
        let started = Instant::now();

        let result = run_helper("sleep", Duration::from_millis(300));

        assert_eq!(result, Err(RunError::TimedOut));
        // Far sooner than the two minutes the helper would have slept.
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn only_the_start_of_a_flood_of_output_is_kept() {
        let ran = run_helper("flood", PLENTY).expect("the helper should run");

        assert!(ran.stdout.len() <= MOST_OUTPUT, "{}", ran.stdout.len());
        assert!(ran.stdout.len() > MOST_OUTPUT / 2, "{}", ran.stdout.len());
    }

    #[test]
    fn a_program_runs_at_the_top_of_the_disk_not_in_the_current_folder() {
        let ran = run_helper("where", PLENTY).expect("the helper should run");
        let here = std::env::current_dir().expect("a current folder");

        let reported = ran
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix("in:"))
            .expect("the helper says where it ran");
        let reported = Path::new(reported);
        assert_ne!(reported, here);
        // No folder is above it, so it can't be inside anybody's project.
        assert!(reported.is_absolute(), "{}", reported.display());
        assert_eq!(reported.parent(), None, "{}", reported.display());
    }

    #[test]
    fn the_neutral_folder_does_not_follow_the_folder_for_temporary_files() {
        let neutral = neutral_folder();

        // `TMPDIR`, or `TEMP` on Windows, can point anywhere, even into a
        // project. The neutral folder is never that folder.
        assert_ne!(neutral, std::env::temp_dir());
        assert!(neutral.is_dir(), "{}", neutral.display());
        // On macOS and Linux it's the same on every computer.
        #[cfg(unix)]
        assert_eq!(neutral, Path::new("/"));
    }

    #[test]
    fn the_top_of_a_disk_is_only_found_from_a_full_path() {
        let here = std::env::current_dir().expect("a current folder");

        let top = top_of_the_disk(Some(here.as_os_str())).expect("the top of this disk");
        assert_eq!(top.parent(), None, "{}", top.display());
        assert!(here.starts_with(&top), "{}", top.display());

        for not_a_full_path in ["", "Windows", "some/folder"] {
            assert_eq!(
                top_of_the_disk(Some(OsStr::new(not_a_full_path))),
                None,
                "{not_a_full_path:?}"
            );
        }
        assert_eq!(top_of_the_disk(None), None);
    }

    #[test]
    fn a_program_that_is_not_there_does_not_start() {
        let missing = std::env::temp_dir().join("wrybill-no-such-program-anywhere");

        assert_eq!(
            run(&missing, &["--version"], PLENTY),
            Err(RunError::NotStarted)
        );
    }

    #[test]
    fn many_jobs_come_back_in_the_order_they_were_given() {
        let job = |does| Job {
            program: this_test_binary(),
            args: helper_args(does),
            limit: PLENTY,
        };
        let jobs = [
            job("print"),
            job("fail"),
            job("print"),
            job("fail"),
            job("print"),
        ];

        let results = run_all(&jobs, 2);

        let succeeded: Vec<bool> = results
            .into_iter()
            .map(|result| result.expect("the helper should run").succeeded)
            .collect();
        assert_eq!(succeeded, [true, false, true, false, true]);
    }

    #[test]
    fn no_jobs_is_no_work() {
        assert_eq!(run_all(&[], 4), []);
    }
}
