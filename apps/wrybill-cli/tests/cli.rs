//! End-to-end checks that run the real `wrybill` binary.

use std::process::{Command, Output};

/// Runs the `wrybill` binary that Cargo built for this test run.
fn wrybill(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wrybill"))
        .args(args)
        .output()
        .expect("the wrybill binary should start")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

#[test]
fn version_prints_the_name_and_version() {
    let output = wrybill(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        stdout(&output).trim_end(),
        concat!("wrybill ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_shows_how_to_call_it() {
    let output = wrybill(&["--help"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Usage: wrybill"));
}

#[test]
fn no_arguments_prints_help() {
    let output = wrybill(&[]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Usage: wrybill"));
}

#[test]
fn an_unknown_argument_is_an_error() {
    let output = wrybill(&["--no-such-flag"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
