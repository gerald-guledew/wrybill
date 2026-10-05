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
