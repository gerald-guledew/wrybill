//! Loading the config from a real file.

use std::fs;
use std::path::{Path, PathBuf};

use wrybill_config::{Config, LoadError, Source, load};

use crate::support::home;

/// A fresh, empty folder for one test, inside Cargo's own temp folder.
fn folder(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("config-files")
        .join(test);
    // Left over from an earlier run, if it's there at all.
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).expect("a temp folder");
    folder
}

fn problems(error: LoadError) -> Vec<String> {
    match error {
        LoadError::Invalid(invalid) => invalid.errors.iter().map(ToString::to_string).collect(),
        LoadError::Unreadable(reason) => {
            panic!("expected problems, but the file was unreadable: {reason}")
        }
    }
}

#[test]
fn a_missing_file_is_not_an_error_and_gives_the_built_in_defaults() {
    let path = folder("missing-file").join("config.toml");

    let loaded = load(&path, Some(&home())).expect("defaults");

    assert_eq!(loaded.source, Source::Defaults);
    assert_eq!(loaded.config, Config::defaults(Some(&home())));
    assert!(loaded.warnings.is_empty());
}

#[test]
fn a_missing_folder_is_not_an_error_either() {
    let path = folder("missing-folder")
        .join("not-created")
        .join("config.toml");

    let loaded = load(&path, Some(&home())).expect("defaults");

    assert_eq!(loaded.source, Source::Defaults);
}

#[test]
fn a_valid_file_is_loaded() {
    let path = folder("valid-file").join("config.toml");
    fs::write(&path, "version = 1\n[limits]\nmax_steps_per_task = 99\n").expect("a file");

    let loaded = load(&path, Some(&home())).expect("a valid config");

    assert_eq!(loaded.source, Source::File);
    assert_eq!(loaded.config.limits.max_steps_per_task, 99);
}

#[test]
fn a_file_with_problems_reports_them_all() {
    let path = folder("invalid-file").join("config.toml");
    fs::write(
        &path,
        "version = 1\n[limits]\nmax_steps_per_task = 0\nmax_minutes_per_task = 0\n",
    )
    .expect("a file");

    let error = load(&path, Some(&home())).expect_err("an invalid config");

    assert_eq!(
        problems(error),
        [
            "line 3: [limits] max_steps_per_task: must be 1 or more, but it's 0 here.",
            "line 4: [limits] max_minutes_per_task: must be 1 or more, but it's 0 here.",
        ]
    );
}

#[test]
fn a_file_saved_by_a_windows_editor_reads_the_same() {
    // A byte-order mark at the start, and CRLF line endings.
    let path = folder("windows-editor").join("config.toml");
    fs::write(
        &path,
        "\u{feff}version = 1\r\n[limits]\r\nmax_steps_per_task = 0\r\n",
    )
    .expect("a file");

    let error = load(&path, Some(&home())).expect_err("an invalid config");

    assert_eq!(
        problems(error),
        ["line 3: [limits] max_steps_per_task: must be 1 or more, but it's 0 here."]
    );
}

#[test]
fn a_file_that_is_not_text_cannot_be_read() {
    let path = folder("not-text").join("config.toml");
    fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).expect("a file");

    let error = load(&path, Some(&home())).expect_err("an unreadable file");

    assert_eq!(
        error.to_string(),
        "the config file can't be read: it isn't plain UTF-8 text"
    );
}

#[test]
fn a_folder_where_the_file_should_be_cannot_be_read() {
    let path = folder("a-folder").join("config.toml");
    fs::create_dir(&path).expect("a folder");

    let error = load(&path, Some(&home())).expect_err("an unreadable file");

    assert!(matches!(error, LoadError::Unreadable(_)), "{error:?}");
}
