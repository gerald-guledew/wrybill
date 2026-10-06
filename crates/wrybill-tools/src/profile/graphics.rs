//! The graphics chips, which decide whether a local model has anything
//! better than the main chip to run on (spec section 5).
//!
//! Each OS keeps this somewhere different:
//!
//! - macOS: `system_profiler` is asked, in JSON so the answer doesn't depend
//!   on the user's language.
//! - Windows: the display adapters' names are read from the registry with
//!   `reg`, which starts far faster than PowerShell.
//! - Linux: `/sys/class/drm` is read. No program is run.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::run::Job;
use super::{OsFamily, plain_if_any};

/// How long the question may take. `system_profiler` can be slow on an old
/// Mac with a spinning disk.
const LIMIT: Duration = Duration::from_secs(10);

/// The registry key that lists display adapters.
const WINDOWS_DISPLAY_CLASS: &str =
    r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Where Linux lists graphics cards.
const LINUX_CARDS: &str = "/sys/class/drm";

/// The most graphics chips a profile lists.
const MOST_CHIPS: usize = 8;

/// The graphics chips in this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Graphics {
    /// The chips that were found, by name.
    Found(Vec<String>),
    /// Wrybill couldn't find out.
    Unknown,
}

/// The program to run to find the graphics chips, on an OS that needs one.
///
/// `system_root` is where Windows is installed, such as `C:\Windows`.
pub(super) fn job(family: OsFamily, system_root: Option<&Path>) -> Option<Job> {
    match family {
        OsFamily::MacOs => Some(Job {
            program: PathBuf::from("/usr/sbin/system_profiler"),
            args: &["SPDisplaysDataType", "-json"],
            limit: LIMIT,
        }),
        OsFamily::Windows => Some(Job {
            program: system_root?.join("System32").join("reg.exe"),
            args: &["query", WINDOWS_DISPLAY_CLASS, "/s", "/v", "DriverDesc"],
            limit: LIMIT,
        }),
        OsFamily::Linux | OsFamily::Other => None,
    }
}

/// Works out the graphics chips. `printed` is what the program from [`job`]
/// printed, when there was one and it ran.
pub(super) fn read(family: OsFamily, printed: Option<&str>) -> Graphics {
    let chips = match family {
        OsFamily::MacOs => printed.map(from_system_profiler).unwrap_or_default(),
        OsFamily::Windows => printed.map(from_reg_query).unwrap_or_default(),
        OsFamily::Linux => from_linux_cards(Path::new(LINUX_CARDS)),
        OsFamily::Other => Vec::new(),
    };
    if chips.is_empty() {
        Graphics::Unknown
    } else {
        Graphics::Found(chips)
    }
}

/// Adds a chip to the list, unless it's there already or the list is full.
fn add(chips: &mut Vec<String>, chip: String) {
    if chips.len() < MOST_CHIPS && !chips.contains(&chip) {
        chips.push(chip);
    }
}

/// Reads the JSON from `system_profiler SPDisplaysDataType -json`.
fn from_system_profiler(json: &str) -> Vec<String> {
    let mut chips = Vec::new();
    let Ok(report) = serde_json::from_str::<serde_json::Value>(json) else {
        return chips;
    };
    let cards = report
        .get("SPDisplaysDataType")
        .and_then(serde_json::Value::as_array);
    for card in cards.into_iter().flatten() {
        let text = |key: &str| card.get(key).and_then(serde_json::Value::as_str);
        let Some(name) = text("sppci_model")
            .or_else(|| text("_name"))
            .and_then(plain_if_any)
        else {
            continue;
        };
        // Its own memory, on a Mac whose graphics chip has some. The key's
        // name has changed between versions of macOS.
        let memory = text("spdisplays_vram")
            .or_else(|| text("_spdisplays_vram"))
            .or_else(|| text("spdisplays_vram_shared"))
            .and_then(plain_if_any);
        add(
            &mut chips,
            match memory {
                Some(memory) => format!("{name} ({memory})"),
                None => name,
            },
        );
    }
    chips
}

/// Reads what `reg query ... /s /v DriverDesc` prints: a line such as
/// `    DriverDesc    REG_SZ    Intel(R) HD Graphics 6000` for each adapter.
fn from_reg_query(printed: &str) -> Vec<String> {
    let mut chips = Vec::new();
    for line in printed.lines() {
        let Some(rest) = line.trim_start().strip_prefix("DriverDesc") else {
            continue;
        };
        let Some(name) = rest.trim_start().strip_prefix("REG_SZ") else {
            continue;
        };
        if let Some(name) = plain_if_any(name) {
            add(&mut chips, name);
        }
    }
    chips
}

/// Reads the cards Linux lists under `/sys/class/drm`: who made each one,
/// and the driver that runs it.
fn from_linux_cards(folder: &Path) -> Vec<String> {
    let mut cards: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        // `card0` is a card. `card0-HDMI-A-1` is one of its sockets.
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("card"))
                .is_some_and(|number| {
                    !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
                })
        })
        .collect();
    cards.sort();

    let mut chips = Vec::new();
    for card in cards {
        let device = card.join("device");
        let maker = std::fs::read_to_string(device.join("vendor"))
            .ok()
            .and_then(|id| maker_of(&id));
        let driver = std::fs::read_to_string(device.join("uevent"))
            .ok()
            .and_then(|uevent| {
                uevent
                    .lines()
                    .find_map(|line| line.strip_prefix("DRIVER="))
                    .and_then(plain_if_any)
            });
        let chip = match (maker, driver) {
            (Some(maker), Some(driver)) => format!("{maker} ({driver} driver)"),
            (Some(maker), None) => maker.to_owned(),
            (None, Some(driver)) => format!("{driver} driver"),
            (None, None) => continue,
        };
        add(&mut chips, chip);
    }
    chips
}

/// The maker behind a PCI vendor number such as `0x8086`.
fn maker_of(id: &str) -> Option<&'static str> {
    let id = id.trim().trim_start_matches("0x").to_ascii_lowercase();
    Some(match id.as_str() {
        "8086" => "Intel",
        "10de" => "NVIDIA",
        "1002" | "1022" => "AMD",
        "106b" => "Apple",
        "1414" => "a Microsoft virtual machine",
        "15ad" => "a VMware virtual machine",
        "1af4" | "1234" | "1b36" => "a QEMU virtual machine",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{Graphics, from_linux_cards, from_reg_query, from_system_profiler, job, read};
    use crate::profile::{OsFamily, test_folder};

    #[test]
    fn an_apple_chip_is_read_from_system_profiler() {
        let json = r#"{
          "SPDisplaysDataType" : [
            {
              "_name" : "Apple M5",
              "spdisplays_vendor" : "sppci_vendor_Apple",
              "sppci_bus" : "spdisplays_builtin",
              "sppci_cores" : "10",
              "sppci_model" : "Apple M5"
            }
          ]
        }"#;

        assert_eq!(from_system_profiler(json), ["Apple M5"]);
    }

    #[test]
    fn an_intel_mac_with_two_chips_lists_both_with_their_memory() {
        let json = r#"{"SPDisplaysDataType":[
            {"_name":"Intel Iris Pro","sppci_model":"Intel Iris Pro","spdisplays_vram_shared":"1536 MB"},
            {"_name":"kHW_NVidiaGeForceGT750MItem","sppci_model":"NVIDIA GeForce GT 750M","spdisplays_vram":"2 GB"},
            {"sppci_bus":"no name at all"}
        ]}"#;

        assert_eq!(
            from_system_profiler(json),
            ["Intel Iris Pro (1536 MB)", "NVIDIA GeForce GT 750M (2 GB)"]
        );
    }

    #[test]
    fn output_that_is_not_the_expected_json_gives_nothing() {
        for printed in [
            "",
            "not json",
            "[]",
            r#"{"SPDisplaysDataType": "text"}"#,
            "{}",
        ] {
            assert!(from_system_profiler(printed).is_empty(), "{printed:?}");
        }
    }

    #[test]
    fn display_adapters_are_read_from_a_registry_query() {
        let printed = "\r\n\
            HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}\\0000\r\n\
            \x20   DriverDesc    REG_SZ    Intel(R) HD Graphics 6000\r\n\
            \r\n\
            HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}\\0001\r\n\
            \x20   DriverDesc    REG_SZ    NVIDIA GeForce 940M\r\n\
            \r\n\
            HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}\\0002\r\n\
            \x20   DriverDesc    REG_SZ    Intel(R) HD Graphics 6000\r\n\
            \r\n\
            End of search: 3 match(es) found.\r\n";

        assert_eq!(
            from_reg_query(printed),
            ["Intel(R) HD Graphics 6000", "NVIDIA GeForce 940M"]
        );
    }

    #[test]
    fn a_registry_query_that_found_nothing_gives_nothing() {
        let printed =
            "ERROR: The system was unable to find the specified registry key or value.\r\n";

        assert!(from_reg_query(printed).is_empty());
        assert!(from_reg_query("    DriverDesc    REG_DWORD    0x1\r\n").is_empty());
    }

    /// Writes one card into a stand-in for `/sys/class/drm`.
    fn add_card(folder: &Path, card: &str, vendor: Option<&str>, uevent: Option<&str>) {
        let device = folder.join(card).join("device");
        fs::create_dir_all(&device).expect("the card's folder");
        if let Some(vendor) = vendor {
            fs::write(device.join("vendor"), vendor).expect("vendor");
        }
        if let Some(uevent) = uevent {
            fs::write(device.join("uevent"), uevent).expect("uevent");
        }
    }

    #[test]
    fn linux_cards_are_read_from_their_folders() {
        let folder = test_folder("graphics-linux");
        let cards = folder.path();
        add_card(
            cards,
            "card1",
            Some("0x10de\n"),
            Some("DRIVER=nvidia\nPCI_ID=10DE:1347\n"),
        );
        add_card(cards, "card0", Some("0x8086\n"), Some("DRIVER=i915\n"));
        // A socket on a card, and something that isn't a card at all.
        add_card(
            cards,
            "card0-HDMI-A-1",
            Some("0x8086\n"),
            Some("DRIVER=i915\n"),
        );
        add_card(cards, "renderD128", Some("0x8086\n"), Some("DRIVER=i915\n"));
        // A card on a board with no PCI bus, and one that says nothing.
        add_card(cards, "card2", None, Some("DRIVER=vc4-drm\n"));
        add_card(cards, "card3", None, None);
        // A maker Wrybill doesn't know.
        add_card(cards, "card4", Some("0xabcd\n"), None);

        assert_eq!(
            from_linux_cards(cards),
            [
                "Intel (i915 driver)",
                "NVIDIA (nvidia driver)",
                "vc4-drm driver"
            ]
        );
    }

    #[test]
    fn a_virtual_machines_card_is_named_as_one() {
        let folder = test_folder("graphics-linux-vm");
        // What GitHub's Linux runners report.
        add_card(
            folder.path(),
            "card0",
            Some("0x1414\n"),
            Some("DRIVER=hyperv_drm\n"),
        );

        assert_eq!(
            from_linux_cards(folder.path()),
            ["a Microsoft virtual machine (hyperv_drm driver)"]
        );
    }

    #[test]
    fn a_computer_with_no_cards_folder_has_nothing_to_read() {
        let folder = test_folder("graphics-none");

        assert!(from_linux_cards(&folder.path().join("missing")).is_empty());
    }

    #[test]
    fn each_os_is_asked_in_its_own_way() {
        let windows = Path::new(r"C:\Windows");

        let mac = job(OsFamily::MacOs, None).expect("a job on macOS");
        assert_eq!(mac.program, Path::new("/usr/sbin/system_profiler"));

        let on_windows = job(OsFamily::Windows, Some(windows)).expect("a job on Windows");
        assert!(
            on_windows
                .program
                .ends_with(Path::new("System32").join("reg.exe"))
        );
        // With no Windows folder known, there's no `reg` to run.
        assert_eq!(job(OsFamily::Windows, None), None);

        // Linux is read from files, so no program is needed.
        assert_eq!(job(OsFamily::Linux, None), None);
        assert_eq!(job(OsFamily::Other, None), None);
    }

    #[test]
    fn nothing_found_is_reported_as_unknown() {
        assert_eq!(read(OsFamily::MacOs, None), Graphics::Unknown);
        assert_eq!(read(OsFamily::MacOs, Some("not json")), Graphics::Unknown);
        assert_eq!(read(OsFamily::Windows, Some("")), Graphics::Unknown);
        assert_eq!(read(OsFamily::Other, None), Graphics::Unknown);
        assert_eq!(
            read(
                OsFamily::Windows,
                Some("    DriverDesc    REG_SZ    Some Adapter\r\n")
            ),
            Graphics::Found(vec!["Some Adapter".to_owned()])
        );
    }
}
