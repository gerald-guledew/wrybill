//! Checks the profile against the machine the tests are running on.
//!
//! To see the profile itself, run:
//!
//! ```sh
//! cargo test -p wrybill-tools --test this_machine -- --nocapture
//! ```

use std::time::Duration;

use wrybill_tools::profile::{OsFamily, Profile, collect};

/// Words that can be a computer's name and are also in a normal profile, so
/// finding them proves nothing.
const EVERYDAY_WORDS: [&str; 12] = [
    "apple",
    "arm64",
    "debian",
    "fedora",
    "intel",
    "linux",
    "localhost",
    "macos",
    "ubuntu",
    "windows",
    "x86_64",
    "aarch64",
];

fn profile_text(profile: &Profile) -> String {
    format!("{profile:#?}")
}

#[test]
fn the_profile_of_this_machine_makes_sense() {
    let profile = collect();
    println!("{}", profile_text(&profile));

    assert!(!profile.os.name.is_empty());
    let expected_family = if cfg!(target_os = "macos") {
        OsFamily::MacOs
    } else if cfg!(windows) {
        OsFamily::Windows
    } else if cfg!(target_os = "linux") {
        OsFamily::Linux
    } else {
        OsFamily::Other
    };
    assert_eq!(profile.os.family, expected_family);

    assert!(profile.chip.logical_cores >= 1);
    if let Some(physical) = profile.chip.physical_cores {
        assert!(physical >= 1);
        assert!(physical <= profile.chip.logical_cores);
    }
    assert!(!profile.chip.architecture.is_empty());

    // Wrybill itself is meant for machines with 4 GB or more, but a test
    // runner can be smaller.
    assert!(profile.memory.total_bytes >= 256 * 1024 * 1024);
    assert!(profile.memory.free_bytes <= profile.memory.total_bytes);

    if let Some(disk) = profile.disk {
        assert!(disk.total_bytes > 0);
        assert!(disk.free_bytes <= disk.total_bytes);
    }

    assert!(profile.took < Duration::from_secs(30), "{:?}", profile.took);
}

#[test]
fn the_profile_is_plain_text_that_fits_on_short_lines() {
    let profile = collect();

    let mut texts = vec![profile.os.name.clone(), profile.chip.architecture.clone()];
    texts.extend(profile.os.version.clone());
    texts.extend(profile.os.kernel.clone());
    texts.extend(profile.chip.name.clone());
    for text in texts {
        assert!(text.is_ascii(), "{text:?}");
        assert!(text.len() <= 80, "{text:?}");
        assert!(!text.contains(['\n', '\r', '\t']), "{text:?}");
    }
}

#[test]
fn the_profile_names_neither_the_computer_nor_its_user() {
    let profile = profile_text(&collect()).to_lowercase();

    let user = ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok());
    let computer = sysinfo::System::host_name()
        // `sams-laptop.local` is still `sams-laptop`.
        .map(|name| name.split('.').next().unwrap_or_default().to_owned());

    for (what, name) in [("the user's name", user), ("the computer's name", computer)] {
        let Some(name) = name.map(|name| name.to_lowercase()) else {
            continue;
        };
        // A very short name, or an everyday word, would be found by chance.
        if name.len() < 4 || EVERYDAY_WORDS.contains(&name.as_str()) {
            continue;
        }
        assert!(!profile.contains(&name), "{what} is in the profile");
    }
}
