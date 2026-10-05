//! Wrybill's own log: where it goes and what a line looks like.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tracing::Level;
use wrybill_cli::logging::{self, Logging};
use wrybill_config::Paths;

/// Noon on 5 October 2026, in UTC.
fn noon() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_791_201_600)
}

/// A data folder for one test, inside Cargo's own temp folder. It doesn't
/// exist yet: creating it is part of what's being tested.
fn data_folder(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("logging")
        .join(test)
        .join("wrybill-home");
    // Left over from an earlier run, if it's there at all.
    let _ = fs::remove_dir_all(&folder);
    folder
}

fn paths(data_folder: &Path) -> Paths {
    Paths::resolve(Some(data_folder.into()), None).expect("paths for a full path")
}

/// Writes events through a log opened at `level`, and returns its lines.
fn lines_logged(test: &str, level: Level, events: impl FnOnce()) -> Vec<Value> {
    let paths = paths(&data_folder(test));
    let (file, path) = logging::open(&paths, noon()).expect("a log file");

    tracing::subscriber::with_default(logging::subscriber(file, level), events);

    fs::read_to_string(path)
        .expect("the log file")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON on its own"))
        .collect()
}

#[test]
fn the_log_file_is_named_for_the_utc_day_and_sits_in_the_logs_folder() {
    let folder = data_folder("file-name");
    let paths = paths(&folder);

    let one_second_before_midnight = UNIX_EPOCH + Duration::from_secs(1_791_244_799);
    let midnight = UNIX_EPOCH + Duration::from_secs(1_791_244_800);

    assert_eq!(
        logging::log_file(&paths, noon()),
        folder.join("logs").join("wrybill-2026-10-05.jsonl")
    );
    assert_eq!(
        logging::log_file(&paths, one_second_before_midnight),
        folder.join("logs").join("wrybill-2026-10-05.jsonl")
    );
    assert_eq!(
        logging::log_file(&paths, midnight),
        folder.join("logs").join("wrybill-2026-10-06.jsonl")
    );
}

#[test]
fn opening_the_log_creates_the_data_folder_and_the_file() {
    let folder = data_folder("creates-folders");
    assert!(!folder.exists());

    let (_file, path) = logging::open(&paths(&folder), noon()).expect("a log file");

    assert_eq!(path, folder.join("logs").join("wrybill-2026-10-05.jsonl"));
    assert!(path.is_file());
}

#[test]
fn each_event_is_one_json_object_on_its_own_line() {
    let lines = lines_logged("json-lines", Level::INFO, || {
        tracing::info!(command = "doctor", steps = 3, "started");
        tracing::warn!("something to look at");
    });

    assert_eq!(lines.len(), 2);

    assert_eq!(lines[0]["level"], "INFO");
    assert_eq!(lines[0]["message"], "started");
    assert_eq!(lines[0]["command"], "doctor");
    assert_eq!(lines[0]["steps"], 3);
    let timestamp = lines[0]["timestamp"].as_str().expect("a timestamp");
    assert!(timestamp.ends_with('Z'), "the time is in UTC: {timestamp}");

    assert_eq!(lines[1]["level"], "WARN");
    assert_eq!(lines[1]["message"], "something to look at");
}

#[test]
fn events_below_the_level_are_left_out() {
    let lines = lines_logged("level-info", Level::INFO, || {
        tracing::debug!("detail");
        tracing::info!("kept");
    });

    let messages: Vec<&Value> = lines.iter().map(|line| &line["message"]).collect();
    assert_eq!(messages, ["kept"]);
}

#[test]
fn a_lower_level_lets_more_through() {
    let lines = lines_logged("level-debug", Level::DEBUG, || {
        tracing::trace!("too much");
        tracing::debug!("detail");
        tracing::info!("kept");
    });

    let messages: Vec<&Value> = lines.iter().map(|line| &line["message"]).collect();
    assert_eq!(messages, ["detail", "kept"]);
}

#[test]
fn a_second_run_on_the_same_day_adds_to_the_file() {
    let paths = paths(&data_folder("appends"));

    for message in ["first run", "second run"] {
        let (file, _) = logging::open(&paths, noon()).expect("a log file");
        tracing::subscriber::with_default(logging::subscriber(file, Level::INFO), || {
            tracing::info!("{message}");
        });
    }

    let text = fs::read_to_string(logging::log_file(&paths, noon())).expect("the log file");
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("first run") && text.contains("second run"));
}

#[test]
fn a_log_that_cannot_be_written_is_a_reason_not_a_failure() {
    // A file sits where the data folder should be, so no folder can be made.
    let folder = data_folder("cannot-write");
    fs::create_dir_all(folder.parent().expect("a parent folder")).expect("the parent folder");
    fs::write(&folder, "in the way").expect("a file");

    assert!(logging::open(&paths(&folder), noon()).is_err());
    let Logging::Off { reason } = logging::start(&paths(&folder), Level::INFO) else {
        panic!("the log can't have started");
    };
    assert!(!reason.is_empty());
}

// Folder and file permissions only exist in this form on macOS and Linux.
#[cfg(unix)]
#[test]
fn the_folders_and_the_file_are_for_their_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let folder = data_folder("owner-only");
    let (_file, path) = logging::open(&paths(&folder), noon()).expect("a log file");

    let mode = |path: &Path| fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode(&folder), 0o700, "the data folder");
    assert_eq!(mode(&folder.join("logs")), 0o700, "the logs folder");
    assert_eq!(mode(&path), 0o600, "the log file");
}
