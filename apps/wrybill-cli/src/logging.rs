//! Wrybill's own log.
//!
//! One JSON object per line, in one file per UTC day under the data folder
//! (spec 15.2). It stays on this computer: Wrybill has no telemetry
//! (spec 11.15).
//!
//! Only Wrybill's own events are written: the ones from a crate named
//! `wrybill` or `wrybill_*`. What other crates log is dropped at every level,
//! so nothing a library decides to print can end up in the file. Some
//! libraries do log: the Linux keychain client writes events about the
//! messages it sends.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{Level, Subscriber};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use wrybill_config::Paths;

/// The environment variable that sets the log's level (spec 13.1).
pub const LEVEL_VARIABLE: &str = "WRYBILL_LOG";

/// The level when `WRYBILL_LOG` isn't set.
pub const DEFAULT_LEVEL: Level = Level::INFO;

/// The last day the date arithmetic is asked to handle: 31 December 9999.
const LAST_DAY: u64 = 2_932_896;

/// Whether this run is writing a log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Logging {
    /// Events are going to this file.
    On {
        /// Today's log file.
        file: PathBuf,
    },
    /// There's no log for this run. The command carries on without one.
    Off {
        /// Why the log couldn't be started.
        reason: String,
    },
}

/// `WRYBILL_LOG` is set to something that isn't a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownLevel;

/// Works out the level from the value of `WRYBILL_LOG`.
///
/// Not set, or empty, means the default. The levels are `error`, `warn`,
/// `info`, `debug` and `trace`.
pub fn level_from(value: Option<&str>) -> Result<Level, UnknownLevel> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_LEVEL);
    };
    match value.to_ascii_lowercase().as_str() {
        "error" => Ok(Level::ERROR),
        "warn" => Ok(Level::WARN),
        "info" => Ok(Level::INFO),
        "debug" => Ok(Level::DEBUG),
        "trace" => Ok(Level::TRACE),
        _ => Err(UnknownLevel),
    }
}

/// The log file for the day, in UTC, that holds `now`.
pub fn log_file(paths: &Paths, now: SystemTime) -> PathBuf {
    let (year, month, day) = utc_date(now);
    paths
        .logs_folder()
        .join(format!("wrybill-{year:04}-{month:02}-{day:02}.jsonl"))
}

/// Opens today's log file to add to, creating it and its folders if needed.
///
/// On macOS and Linux the folders and the file are created for their owner
/// only. Folders that are already there are left as they are.
pub fn open(paths: &Paths, now: SystemTime) -> io::Result<(File, PathBuf)> {
    create_private_folder(&paths.logs_folder())?;

    let path = log_file(paths, now);
    let mut options = OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path)?;
    Ok((file, path))
}

fn create_private_folder(folder: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(folder)
}

/// A subscriber that writes each of Wrybill's own events at `level` or above
/// to `file`, as one JSON object per line.
///
/// Events from any other crate are dropped, whatever their level.
pub fn subscriber(file: File, level: Level) -> impl Subscriber + Send + Sync + 'static {
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_max_level(level)
        .with_writer(Mutex::new(file))
        .finish()
        .with(filter_fn(|metadata| is_wrybills_own(metadata.target())))
}

/// Whether an event's target, which says where it comes from, names one of
/// Wrybill's own crates. A target starts with the crate's name, and Wrybill's
/// crates are `wrybill` (the command itself) and `wrybill_*` (AGENTS.md,
/// rule 9).
fn is_wrybills_own(target: &str) -> bool {
    let crate_name = target.split("::").next().unwrap_or(target);
    crate_name == "wrybill" || crate_name.starts_with("wrybill_")
}

/// Starts the log for this run of the command.
///
/// It never stops the command. If the log can't be written, the run carries
/// on without one, and the reason comes back so the command can say so.
pub fn start(paths: &Paths, level: Level) -> Logging {
    let (file, path) = match open(paths, SystemTime::now()) {
        Ok(opened) => opened,
        Err(error) => {
            return Logging::Off {
                reason: error.to_string(),
            };
        }
    };
    match tracing::subscriber::set_global_default(subscriber(file, level)) {
        Ok(()) => Logging::On { file: path },
        Err(_) => Logging::Off {
            reason: "the log was already started".to_owned(),
        },
    }
}

/// The calendar date, in UTC, of a moment in time.
///
/// A clock set before 1970 counts as 1 January 1970.
fn utc_date(moment: SystemTime) -> (u64, u64, u64) {
    let seconds = moment
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    civil_from_days((seconds / 86_400).min(LAST_DAY))
}

/// Turns a count of days since 1 January 1970 into a year, month and day.
///
/// This is Howard Hinnant's `civil_from_days`, for dates from 1970 on:
/// <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    // Count from 1 March 0000, so a leap day is the last day of its year.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;

    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use tracing::Level;

    use super::{UnknownLevel, is_wrybills_own, level_from, utc_date};

    fn date_at(seconds: u64) -> (u64, u64, u64) {
        utc_date(UNIX_EPOCH + Duration::from_secs(seconds))
    }

    #[test]
    fn only_events_from_wrybills_own_crates_count_as_its_own() {
        for target in [
            "wrybill",
            "wrybill_cli",
            "wrybill_cli::doctor",
            "wrybill_config::load",
            "wrybill_tools::profile::run",
        ] {
            assert!(is_wrybills_own(target), "{target}");
        }
        for target in [
            "",
            "zbus",
            "zbus::connection",
            "tokio::runtime",
            // Names that only look like Wrybill's.
            "wrybillish",
            "wrybillish::thing",
            "not_wrybill_cli",
            "other::wrybill_cli",
        ] {
            assert!(!is_wrybills_own(target), "{target}");
        }
    }

    #[test]
    fn known_moments_fall_on_the_right_utc_day() {
        assert_eq!(date_at(0), (1970, 1, 1));
        assert_eq!(date_at(86_399), (1970, 1, 1));
        assert_eq!(date_at(86_400), (1970, 1, 2));
        // The last second of 1999 and the first of 2000.
        assert_eq!(date_at(946_684_799), (1999, 12, 31));
        assert_eq!(date_at(946_684_800), (2000, 1, 1));
        // Leap days: 2000 is a leap year, and so is 2024.
        assert_eq!(date_at(951_782_400), (2000, 2, 29));
        assert_eq!(date_at(1_709_164_800), (2024, 2, 29));
        assert_eq!(date_at(1_709_251_200), (2024, 3, 1));
        assert_eq!(date_at(1_700_000_000), (2023, 11, 14));
        // Noon on 5 October 2026.
        assert_eq!(date_at(1_791_201_600), (2026, 10, 5));
        // 2100 is not a leap year.
        assert_eq!(date_at(4_107_456_000), (2100, 2, 28));
        assert_eq!(date_at(4_107_542_400), (2100, 3, 1));
    }

    #[test]
    fn a_clock_set_before_1970_counts_as_the_first_day() {
        assert_eq!(utc_date(UNIX_EPOCH - Duration::from_secs(1)), (1970, 1, 1));
    }

    #[test]
    fn a_clock_set_absurdly_far_ahead_stops_at_the_year_9999() {
        // About the year 21000, which every OS's clock can still hold.
        assert_eq!(date_at(600_000_000_000), (9999, 12, 31));
    }

    #[test]
    fn the_level_is_info_unless_wrybill_log_says_otherwise() {
        assert_eq!(level_from(None), Ok(Level::INFO));
        assert_eq!(level_from(Some("")), Ok(Level::INFO));
        assert_eq!(level_from(Some("  ")), Ok(Level::INFO));
    }

    #[test]
    fn wrybill_log_takes_the_five_levels() {
        assert_eq!(level_from(Some("error")), Ok(Level::ERROR));
        assert_eq!(level_from(Some("warn")), Ok(Level::WARN));
        assert_eq!(level_from(Some("info")), Ok(Level::INFO));
        assert_eq!(level_from(Some("debug")), Ok(Level::DEBUG));
        assert_eq!(level_from(Some("trace")), Ok(Level::TRACE));
        assert_eq!(level_from(Some(" Debug ")), Ok(Level::DEBUG));
    }

    #[test]
    fn anything_else_in_wrybill_log_is_not_a_level() {
        for value in ["verbose", "warning", "3", "debug,trace"] {
            assert_eq!(level_from(Some(value)), Err(UnknownLevel), "{value}");
        }
    }
}
