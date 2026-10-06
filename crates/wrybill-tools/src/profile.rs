//! The system profile: what this computer is and what's on it (spec 10.3).
//!
//! `wrybill doctor` prints it. From M1 the `system.profile` tool hands the
//! same profile to a brain, so plans fit the machine.
//!
//! A profile is worked out fresh each time [`collect`] is called. Most of it
//! comes from asking the OS or from looking for files. A short, fixed list of
//! programs is run, each to answer one question such as its version. Nothing
//! is sent anywhere.
//!
//! A profile never holds the computer's name, the user's name, a serial
//! number, a network address or a path, so it's safe to print and to paste
//! into a public issue.

mod date;
mod features;
mod graphics;
mod installed;
mod network;
mod path_search;
mod run;
mod system;
mod updates;

use std::time::{Duration, Instant, SystemTime};

pub use self::date::Date;
pub use self::features::CpuFeatures;
pub use self::graphics::Graphics;
pub use self::installed::{Installed, Runtime, RuntimeState, Shells};
pub use self::network::Network;
pub use self::updates::{OsRelease, SecurityUpdates, TABLE_CHECKED, WhyUnknown, security_updates};

/// The longest piece of text taken from the OS that a profile keeps.
const MOST_TEXT: usize = 80;

/// What this computer is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The operating system.
    pub os: Os,
    /// Whether the OS still gets security updates.
    pub security_updates: SecurityUpdates,
    /// The chip.
    pub chip: Chip,
    /// The memory.
    pub memory: Memory,
    /// The disk that holds the user's home folder, when it can be found.
    pub disk: Option<Disk>,
    /// Whether the OS has a route out to the internet.
    pub network: Network,
    /// The shells, package managers, runtimes, browsers and graphics chips.
    pub installed: Installed,
    /// How long the profile took to work out.
    pub took: Duration,
}

/// The operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Os {
    /// Which family it belongs to.
    pub family: OsFamily,
    /// Its name, such as `macOS`, `Windows 11 Pro` or `Ubuntu`.
    pub name: String,
    /// Its version, such as `26.7.1`, `build 26100` or `24.04`.
    pub version: Option<String>,
    /// The kernel's version. Only given on Linux, where it says something
    /// the distribution's version doesn't.
    pub kernel: Option<String>,
}

/// The family an operating system belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsFamily {
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
    /// Linux.
    Linux,
    /// Anything else.
    Other,
}

impl OsFamily {
    /// The family of the OS this program was built for.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// The chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// The maker's name for it, when the OS knows one.
    pub name: Option<String>,
    /// Its kind, such as `x86_64` or `arm64`.
    pub architecture: String,
    /// How many cores it really has, when the OS knows.
    pub physical_cores: Option<usize>,
    /// How many cores the OS can schedule work on.
    pub logical_cores: usize,
    /// What it can do that matters for running models locally.
    pub features: CpuFeatures,
}

/// The memory, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    /// How much the computer has.
    pub total_bytes: u64,
    /// How much a program could use right now without pushing others out.
    pub free_bytes: u64,
}

/// A disk, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disk {
    /// How much it holds.
    pub total_bytes: u64,
    /// How much is free for the user.
    pub free_bytes: u64,
    /// Whether it's a solid-state drive or a spinning one.
    pub kind: DiskKind,
}

/// The kind of a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskKind {
    /// A solid-state drive.
    Ssd,
    /// A spinning hard drive. Many old laptops still have one (spec section 5).
    Hdd,
    /// The OS doesn't say.
    Unknown,
}

/// Works out the profile of this computer.
pub fn collect() -> Profile {
    let started = Instant::now();
    let today = date::days_since_1970(SystemTime::now());

    let (os, security_updates) = system::os(today);
    let (chip, memory) = system::chip_and_memory();
    let disk = system::disk_holding(std::env::home_dir().as_deref());
    let network = network::check();
    let installed = installed::collect(os.family);

    Profile {
        os,
        security_updates,
        chip,
        memory,
        disk,
        network,
        installed,
        took: started.elapsed(),
    }
}

/// Tidies text that came from the OS or from another program, so it can be
/// printed anywhere: plain ASCII on one line, and not too long. Anything else
/// becomes a `?`.
fn plain(text: &str) -> String {
    let mut tidy = String::new();
    for word in text.split_whitespace() {
        if !tidy.is_empty() {
            tidy.push(' ');
        }
        tidy.extend(word.chars().map(|letter| {
            if letter.is_ascii_graphic() {
                letter
            } else {
                '?'
            }
        }));
    }
    // Only ASCII is left, so any length is a whole number of characters.
    tidy.truncate(MOST_TEXT);
    tidy
}

/// The same, or `None` when nothing is left.
fn plain_if_any(text: &str) -> Option<String> {
    Some(plain(text)).filter(|tidy| !tidy.is_empty())
}

/// A folder for one test to make files in. It's removed when the test ends.
#[cfg(test)]
pub(crate) struct TestFolder(std::path::PathBuf);

#[cfg(test)]
impl TestFolder {
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        // The folder the tests share goes too, once it's empty.
        if let Some(shared) = self.0.parent() {
            let _ = std::fs::remove_dir(shared);
        }
    }
}

/// A folder for the test called `test`, inside the OS's folder for temporary
/// files. It doesn't exist yet.
#[cfg(test)]
pub(crate) fn test_folder(test: &str) -> TestFolder {
    let folder = std::env::temp_dir()
        .join(format!("wrybill-tools-tests-{}", std::process::id()))
        .join(test);
    // Left over from an earlier run, if it's there at all.
    let _ = std::fs::remove_dir_all(&folder);
    TestFolder(folder)
}

#[cfg(test)]
mod tests {
    use super::{MOST_TEXT, plain, plain_if_any};

    #[test]
    fn text_from_the_os_is_tidied_onto_one_plain_line() {
        assert_eq!(
            plain("  Intel(R) Core(TM) i5-5250U CPU @ 1.60GHz  "),
            "Intel(R) Core(TM) i5-5250U CPU @ 1.60GHz"
        );
        assert_eq!(plain("two\nlines\tand a tab"), "two lines and a tab");
        assert_eq!(plain("caf\u{e9} \u{1F600}"), "caf? ?");
        assert_eq!(plain("bell\u{7}here"), "bell?here");
    }

    #[test]
    fn long_text_is_cut_short() {
        assert_eq!(plain(&"x".repeat(500)).len(), MOST_TEXT);
    }

    #[test]
    fn text_with_nothing_in_it_counts_as_missing() {
        assert_eq!(plain_if_any("   "), None);
        assert_eq!(plain_if_any(""), None);
        assert_eq!(plain_if_any(" Apple M5 "), Some("Apple M5".to_owned()));
    }
}
