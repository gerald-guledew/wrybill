//! End-to-end checks that run the real `wrybill` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A data folder for one test, inside Cargo's own temp folder. It doesn't
/// exist yet, and the real `~/.wrybill` is never touched.
fn data_folder(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli")
        .join(test)
        .join("wrybill-home");
    // Left over from an earlier run, if it's there at all.
    let _ = fs::remove_dir_all(&folder);
    folder
}

/// Runs the `wrybill` binary that Cargo built for this test run, with its
/// data folder moved to `data_folder`.
fn wrybill(data_folder: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wrybill"))
        .args(args)
        .env("WRYBILL_HOME", data_folder)
        .output()
        .expect("the wrybill binary should start")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

#[test]
fn version_prints_the_name_and_version() {
    let output = wrybill(&data_folder("version"), &["--version"]);

    assert!(output.status.success());
    assert_eq!(
        stdout(&output).trim_end(),
        concat!("wrybill ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_shows_how_to_call_it() {
    let output = wrybill(&data_folder("help"), &["--help"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Usage: wrybill"));
}

#[test]
fn no_arguments_prints_help() {
    let output = wrybill(&data_folder("no-arguments"), &[]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Usage: wrybill"));
}

#[test]
fn an_unknown_argument_is_an_error() {
    let output = wrybill(&data_folder("unknown-argument"), &["--no-such-flag"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn help_and_version_create_no_files_or_folders() {
    let folder = data_folder("creates-nothing");
    let parent = folder.parent().expect("a parent folder");
    fs::create_dir_all(parent).expect("the parent folder");

    for args in [&["--help"][..], &["--version"], &[], &["--no-such-flag"]] {
        wrybill(&folder, args);

        assert!(!folder.exists(), "the data folder appeared after {args:?}");
        let left_behind: Vec<_> = fs::read_dir(parent).expect("the parent folder").collect();
        assert!(left_behind.is_empty(), "{left_behind:?} after {args:?}");
    }
}

// `wrybill keys set`, as far as it goes without touching a keychain: every
// run below is turned away before anything is saved.

/// Something shaped like a key. No test may ever find it in what the command
/// prints or logs.
const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

/// Everything in the log files under a data folder.
fn log(data_folder: &Path) -> String {
    let Ok(files) = fs::read_dir(data_folder.join("logs")) else {
        return String::new();
    };
    files
        .map(|file| fs::read_to_string(file.expect("a log file").path()).expect("log text"))
        .collect()
}

fn assert_the_key_is_nowhere(output: &Output, data_folder: &Path) {
    for (place, text) in [
        ("stdout", stdout(output)),
        ("stderr", stderr(output)),
        ("the log", log(data_folder)),
    ] {
        assert!(!text.contains("MARKER"), "the key is in {place}:\n{text}");
    }
}

#[test]
fn a_key_on_the_command_line_is_refused() {
    let folder = data_folder("key-on-the-command-line");

    let output = wrybill(&folder, &["keys", "set", "anthropic", MARKER]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).starts_with("A key doesn't go on the command line."),
        "{}",
        stderr(&output)
    );
    assert!(
        log(&folder).contains("nothing was saved"),
        "{}",
        log(&folder)
    );
    assert_the_key_is_nowhere(&output, &folder);
}

#[test]
fn a_flag_and_a_key_after_the_name_are_refused_the_same_way() {
    let folder = data_folder("flag-and-key");

    let output = wrybill(&folder, &["keys", "set", "anthropic", "--key", MARKER]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).starts_with("A key doesn't go on the command line."),
        "{}",
        stderr(&output)
    );
    assert_the_key_is_nowhere(&output, &folder);
}

#[test]
fn a_key_where_the_name_goes_is_refused_and_not_echoed() {
    let folder = data_folder("key-as-name");
    let key_as_name = MARKER.to_uppercase();

    let output = wrybill(&folder, &["keys", "set", &key_as_name]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).starts_with("That isn't a name a key can be saved under."),
        "{}",
        stderr(&output)
    );
    assert_the_key_is_nowhere(&output, &folder);
}

#[test]
fn piping_in_nothing_saves_nothing() {
    // The tests give the command no input, which is what an empty pipe is.
    let folder = data_folder("nothing-piped");

    let output = wrybill(&folder, &["keys", "set", "anthropic"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "No key was given, so nothing was saved.\n");
}

#[test]
fn keys_on_its_own_shows_what_it_can_do() {
    let folder = data_folder("keys-alone");

    let output = wrybill(&folder, &["keys"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Usage: wrybill keys <COMMAND>"));
    assert!(!folder.exists(), "showing help created the data folder");
}

#[test]
fn a_log_level_that_is_not_one_gets_a_note() {
    let folder = data_folder("unknown-level");

    let output = Command::new(env!("CARGO_BIN_EXE_wrybill"))
        .args(["keys", "set", "searxng"])
        .env("WRYBILL_HOME", &folder)
        .env("WRYBILL_LOG", "loud")
        .output()
        .expect("the wrybill binary should start");

    assert!(
        stderr(&output).starts_with(
            "Note: WRYBILL_LOG isn't one of error, warn, info, debug or trace, so the log is at info.\n"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_data_folder_that_cannot_be_used_gets_a_note_and_the_command_carries_on() {
    // WRYBILL_HOME isn't a full path, so there's nowhere to keep a log.
    let output = Command::new(env!("CARGO_BIN_EXE_wrybill"))
        .args(["keys", "set", "searxng"])
        .env("WRYBILL_HOME", "not-a-full-path")
        .output()
        .expect("the wrybill binary should start");

    assert_eq!(
        stderr(&output),
        "Note: Wrybill isn't keeping a log of this run. Reason: WRYBILL_HOME must be the full path of a folder. It can't start from the current folder or with ~.\n\
         SearXNG takes no key, so there's nothing to save.\n"
    );
    assert_eq!(output.status.code(), Some(2));
}
