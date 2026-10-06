//! `wrybill doctor`, tested without starting a process: the setup checks
//! against folders made for each test and a keychain in memory, and the
//! layout against profiles written out by hand.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use wrybill_cli::doctor::{
    Build, ConfigCheck, KeyLine, KeychainCheck, KeysCheck, LogsCheck, NOT_USABLE, Report, Setup,
    USABLE, check_setup, render,
};
use wrybill_config::{
    KeyRef, KeyStatus, MemoryStore, Paths, PathsError, Problem, Secret, SecretStore, StoreError,
};
use wrybill_tools::profile::{
    Chip, CpuFeatures, Date, Disk, DiskKind, Graphics, Installed, Memory, Network, Os, OsFamily,
    Profile, Runtime, RuntimeState, SecurityUpdates, Shells, WhyUnknown,
};

/// Something shaped like a key. No test may ever find it in what doctor
/// prints.
const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

const GB: u64 = 1024 * 1024 * 1024;

fn runtime(name: &'static str, state: RuntimeState) -> Runtime {
    Runtime { name, state }
}

fn version(text: &str) -> RuntimeState {
    RuntimeState::Found {
        version: Some(text.to_owned()),
    }
}

fn keychain_key(name: &str) -> KeyRef {
    KeyRef::keychain(name).expect("a valid key name")
}

fn env_key(variable: &str) -> KeyRef {
    KeyRef::Env {
        variable: variable.to_owned(),
    }
}

/// A Mac like the one Wrybill is developed on, with nothing wrong.
fn healthy_mac() -> Report {
    Report {
        build: Build {
            version: "0.1.0",
            commit: None,
            target: "aarch64-apple-darwin",
        },
        setup: Setup {
            config: ConfigCheck::Defaults {
                file: "~/.wrybill/config.toml".to_owned(),
            },
            keychain: KeychainCheck::Reachable("the macOS Keychain"),
            keys: KeysCheck::Checked(Vec::new()),
            logs: LogsCheck::Kept {
                folder: "~/.wrybill/logs".to_owned(),
                files: 1,
                bytes: 129,
            },
        },
        profile: Profile {
            os: Os {
                family: OsFamily::MacOs,
                name: "macOS".to_owned(),
                version: Some("26.7.1".to_owned()),
                kernel: None,
            },
            security_updates: SecurityUpdates::Supported { until: None },
            chip: Chip {
                name: Some("Apple M5".to_owned()),
                architecture: "arm64".to_owned(),
                physical_cores: Some(10),
                logical_cores: 10,
                features: CpuFeatures {
                    avx: None,
                    avx2: None,
                    others: vec!["NEON", "DotProd", "I8MM"],
                },
            },
            memory: Memory {
                total_bytes: 32 * GB,
                free_bytes: 23 * GB,
            },
            disk: Some(Disk {
                total_bytes: 494_332_366_848,
                free_bytes: 187_629_075_578,
                kind: DiskKind::Ssd,
            }),
            network: Network::Up,
            installed: Installed {
                shells: Shells {
                    login: Some("zsh".to_owned()),
                    found: vec!["sh", "bash", "zsh"],
                },
                package_managers: vec!["Homebrew"],
                runtimes: vec![
                    runtime("git", version("2.50.1")),
                    runtime("Python", version("3.9.6")),
                    runtime("Node.js", version("24.13.0")),
                    runtime("Java", version("25.0.4.1")),
                    runtime("Docker", RuntimeState::NotInstalled),
                ],
                browsers: vec!["Chrome"],
                graphics: Graphics::Found(vec!["Apple M5".to_owned()]),
            },
            took: Duration::from_millis(289),
        },
    }
}

/// A 2015 laptop on Windows 10, with a valid config and a few things to fix.
fn old_windows_laptop() -> Report {
    Report {
        build: Build {
            version: "0.1.0",
            commit: Some("3b22765f0a1b2c3d4e5f60718293a4b5c6d7e8f9"),
            target: "x86_64-pc-windows-msvc",
        },
        setup: Setup {
            config: ConfigCheck::Valid {
                file: r"~\.wrybill\config.toml".to_owned(),
                warnings: vec![Problem {
                    line: Some(21),
                    place: "[[models]] entry 3 (\"gpt\"), min_free_ram_gb".to_owned(),
                    message: "only matters for a local model, so it's ignored here.".to_owned(),
                }],
            },
            keychain: KeychainCheck::Reachable("Windows Credential Manager"),
            keys: KeysCheck::Checked(vec![
                KeyLine {
                    reference: keychain_key("anthropic"),
                    models: 2,
                    search: false,
                    status: KeyStatus::Found,
                },
                KeyLine {
                    reference: env_key("OPENAI_API_KEY"),
                    models: 1,
                    search: false,
                    status: KeyStatus::Missing,
                },
                KeyLine {
                    reference: keychain_key("brave"),
                    models: 0,
                    search: true,
                    status: KeyStatus::Missing,
                },
            ]),
            logs: LogsCheck::Kept {
                folder: r"~\.wrybill\logs".to_owned(),
                files: 12,
                bytes: 3 * 1024 * 1024 + 300 * 1024,
            },
        },
        profile: Profile {
            os: Os {
                family: OsFamily::Windows,
                name: "Windows 10 Home".to_owned(),
                version: Some("build 19045".to_owned()),
                kernel: None,
            },
            security_updates: SecurityUpdates::SupportedIf {
                condition: "only with Extended Security Updates (ESU), which need enrolling in",
                until: Some(Date::new(2027, 10, 12)),
            },
            chip: Chip {
                name: Some("Intel(R) Core(TM) i5-5200U CPU @ 2.20GHz".to_owned()),
                architecture: "x86_64".to_owned(),
                physical_cores: Some(2),
                logical_cores: 4,
                features: CpuFeatures {
                    avx: Some(true),
                    avx2: Some(true),
                    others: vec!["SSE4.2", "FMA", "F16C"],
                },
            },
            memory: Memory {
                total_bytes: 8 * GB - 100 * 1024 * 1024,
                free_bytes: 3 * GB,
            },
            disk: Some(Disk {
                total_bytes: 500_107_862_016,
                free_bytes: 7_500_000_000,
                kind: DiskKind::Hdd,
            }),
            network: Network::NoRoute,
            installed: Installed {
                shells: Shells {
                    login: None,
                    found: vec!["cmd", "Windows PowerShell"],
                },
                package_managers: vec!["winget"],
                runtimes: vec![
                    runtime("git", version("2.47.1.windows.2")),
                    runtime("Python", RuntimeState::NotInstalled),
                    runtime("Node.js", RuntimeState::NoAnswer),
                    runtime("Java", RuntimeState::Found { version: None }),
                    runtime("Docker", RuntimeState::NotInstalled),
                ],
                browsers: vec!["Edge", "Firefox"],
                graphics: Graphics::Found(vec![
                    "Intel(R) HD Graphics 5500".to_owned(),
                    "NVIDIA GeForce 940M".to_owned(),
                ]),
            },
            took: Duration::from_millis(2_449),
        },
    }
}

/// A 2015 MacBook that was never set up for development, with a config
/// that has a key pasted into it.
fn bare_old_mac() -> Report {
    Report {
        build: Build {
            version: "0.1.0",
            commit: Some("3b22765f0a1b2c3d4e5f60718293a4b5c6d7e8f9"),
            target: "x86_64-apple-darwin",
        },
        setup: Setup {
            config: ConfigCheck::Invalid {
                file: "~/.wrybill/config.toml".to_owned(),
                errors: vec![
                    Problem {
                        line: Some(7),
                        place: "[[models]] entry 1 (\"claude\"), api_key".to_owned(),
                        message: "must point to a key, not hold one.".to_owned(),
                    },
                    Problem {
                        line: Some(9),
                        place: "[limits] max_steps_per_task".to_owned(),
                        message: "must be 1 or more, but it's 0 here.".to_owned(),
                    },
                ],
                warnings: Vec::new(),
            },
            keychain: KeychainCheck::Failed("the macOS Keychain"),
            keys: KeysCheck::NotChecked,
            logs: LogsCheck::Empty {
                folder: "~/.wrybill/logs".to_owned(),
            },
        },
        profile: Profile {
            os: Os {
                family: OsFamily::MacOs,
                name: "macOS".to_owned(),
                version: Some("12.7.6".to_owned()),
                kernel: None,
            },
            security_updates: SecurityUpdates::OutOfSupport {
                since: Date::new(2024, 7, 29),
                or_earlier: false,
            },
            chip: Chip {
                name: Some("Intel(R) Core(TM) i5-5250U CPU @ 1.60GHz".to_owned()),
                architecture: "x86_64".to_owned(),
                physical_cores: Some(2),
                logical_cores: 4,
                features: CpuFeatures {
                    avx: Some(true),
                    avx2: Some(true),
                    others: Vec::new(),
                },
            },
            memory: Memory {
                total_bytes: 8 * GB,
                free_bytes: 2 * GB,
            },
            disk: None,
            network: Network::Up,
            installed: Installed {
                shells: Shells {
                    login: Some("fish".to_owned()),
                    found: vec!["sh", "bash", "zsh"],
                },
                package_managers: Vec::new(),
                runtimes: vec![
                    runtime("git", RuntimeState::NeedsDeveloperTools),
                    runtime("Python", RuntimeState::NeedsDeveloperTools),
                    runtime("Node.js", RuntimeState::NotInstalled),
                    runtime("Java", RuntimeState::NotInstalled),
                    runtime("Docker", RuntimeState::NotInstalled),
                ],
                browsers: Vec::new(),
                graphics: Graphics::Unknown,
            },
            took: Duration::from_millis(40),
        },
    }
}

/// A small Linux server reached over SSH, where no keychain can be used.
fn linux_server() -> Report {
    Report {
        build: Build {
            version: "0.1.0",
            commit: Some("3b22765f0a1b2c3d4e5f60718293a4b5c6d7e8f9"),
            target: "aarch64-unknown-linux-musl",
        },
        setup: Setup {
            config: ConfigCheck::Unreadable {
                file: "$WRYBILL_HOME/config.toml".to_owned(),
                reason: "it isn't plain UTF-8 text".to_owned(),
            },
            keychain: KeychainCheck::Unreachable("the Secret Service keyring"),
            keys: KeysCheck::NotChecked,
            logs: LogsCheck::Kept {
                folder: "$WRYBILL_HOME/logs".to_owned(),
                files: 3,
                bytes: 20_000,
            },
        },
        profile: Profile {
            os: Os {
                family: OsFamily::Linux,
                name: "Ubuntu".to_owned(),
                version: Some("24.04".to_owned()),
                kernel: Some("6.8.0-45-generic".to_owned()),
            },
            security_updates: SecurityUpdates::Unknown(WhyUnknown::OsNotCovered),
            chip: Chip {
                name: None,
                architecture: "aarch64".to_owned(),
                physical_cores: None,
                logical_cores: 1,
                features: CpuFeatures {
                    avx: None,
                    avx2: None,
                    others: Vec::new(),
                },
            },
            memory: Memory {
                total_bytes: GB,
                free_bytes: GB / 2,
            },
            disk: Some(Disk {
                total_bytes: 8_000_000_000,
                free_bytes: 2_100_000_000,
                kind: DiskKind::Unknown,
            }),
            network: Network::Up,
            installed: Installed {
                shells: Shells {
                    login: Some("bash".to_owned()),
                    found: vec!["sh", "bash", "dash"],
                },
                package_managers: vec!["apt"],
                runtimes: vec![
                    runtime("git", version("2.43.0")),
                    runtime("Python", version("3.12.3")),
                    runtime("Node.js", RuntimeState::NotInstalled),
                    runtime("Java", RuntimeState::NotInstalled),
                    runtime("Docker", version("27.5.1")),
                ],
                browsers: Vec::new(),
                graphics: Graphics::Unknown,
            },
            took: Duration::from_millis(612),
        },
    }
}

const HEALTHY_MAC: &str = r#"wrybill 0.1.0 (a local build, aarch64-apple-darwin)

Setup
  Config file:      OK. There's no ~/.wrybill/config.toml, so Wrybill is using
                    its built-in defaults.
  Keychain:         OK. The macOS Keychain can be reached.
  Keys:             None needed. Nothing in the config uses a key yet.
  Logs:             1 file, 129 bytes, in ~/.wrybill/logs

This computer
  OS:               macOS 26.7.1
  Security updates: OK. This version still gets security updates.
                    Wrybill's table of OS versions was checked on 6 October
                    2026.
  Chip:             Apple M5, 10 cores (arm64)
  CPU features:     NEON, DotProd, I8MM. AVX and AVX2 don't apply to this kind
                    of chip.
  Memory:           32.0 GB in total, 23.0 GB free
  Disk:             494 GB in total, 188 GB free, an SSD (the disk your home
                    folder is on)
  Graphics:         Apple M5
  Network:          OK. There's a route to the internet. Nothing was sent to
                    test it.

Installed
  Shells:           sh, bash, zsh (your login shell)
  Package managers: Homebrew
  git:              2.50.1
  Python:           3.9.6
  Node.js:          24.13.0
  Java:             25.0.4.1
  Docker:           Not installed
  Browsers:         Chrome

The profile took 0.3 seconds.
"#;

const OLD_WINDOWS_LAPTOP: &str = r#"wrybill 0.1.0 (commit 3b22765f0a1b, x86_64-pc-windows-msvc)

Setup
  Config file:      OK. ~\.wrybill\config.toml is valid.
                    Warning: line 21: [[models]] entry 3 ("gpt"),
                    min_free_ram_gb: only matters for a local model, so it's
                    ignored here.
  Keychain:         OK. Windows Credential Manager can be reached.
  Keys:             keychain:wrybill/anthropic: OK, used by 2 models.
                    env:OPENAI_API_KEY: Missing, used by 1 model. The variable
                    isn't set, or it's empty.
                    Next: set OPENAI_API_KEY in the environment Wrybill runs
                    in.
                    Warning: this key is kept in an environment variable.
                    That's meant for machines with no keychain to use.
                    keychain:wrybill/brave: Missing, used by web search.
                    Next: wrybill keys set brave
  Logs:             12 files, 3.3 MB, in ~\.wrybill\logs

This computer
  OS:               Windows 10 Home (build 19045)
  Security updates: Warning. This version gets security updates only with
                    Extended Security Updates (ESU), which need enrolling in,
                    until 12 October 2027.
                    Next: open Settings, then Windows Update, and check that
                    this PC is enrolled.
                    Wrybill's table of OS versions was checked on 6 October
                    2026.
  Chip:             Intel(R) Core(TM) i5-5200U CPU @ 2.20GHz, 2 cores, 4
                    threads (x86_64)
  CPU features:     AVX: yes. AVX2: yes. Also: SSE4.2, FMA, F16C.
  Memory:           7.9 GB in total, 3.0 GB free
  Disk:             500 GB in total, 7.5 GB free, a spinning hard drive (the
                    disk your home folder is on)
  Graphics:         Intel(R) HD Graphics 5500, NVIDIA GeForce 940M
  Network:          Warning. There's no route to the internet. Wrybill sticks
                    to local and LAN brains, and web tools stay off, until the
                    connection is back.

Installed
  Shells:           cmd, Windows PowerShell
  Package managers: winget
  git:              2.47.1.windows.2
  Python:           Not installed
  Node.js:          Warning. It's there, but it didn't answer in time.
  Java:             Installed, but Wrybill couldn't read its version.
  Docker:           Not installed
  Browsers:         Edge, Firefox

The profile took 2.4 seconds.
"#;

const BARE_OLD_MAC: &str = r#"wrybill 0.1.0 (commit 3b22765f0a1b, x86_64-apple-darwin)

Setup
  Config file:      Problem. ~/.wrybill/config.toml has 2 errors, so Wrybill
                    can't use it:
                    - line 7: [[models]] entry 1 ("claude"), api_key: must
                      point to a key, not hold one.
                    - line 9: [limits] max_steps_per_task: must be 1 or more,
                      but it's 0 here.
                    Next: fix them in the file, then run wrybill doctor again.
  Keychain:         Warning. The macOS Keychain reported a problem. Today's
                    log file has the details.
  Keys:             Not checked, because the config file can't be used.
  Logs:             None yet. They will go in ~/.wrybill/logs

This computer
  OS:               macOS 12.7.6
  Security updates: Warning. Out of support: no security updates since 29 July
                    2024.
                    Next: update the OS if this computer can run a newer one.
                    For an old laptop, a current Linux distribution is the
                    safest home.
                    Wrybill's table of OS versions was checked on 6 October
                    2026.
  Chip:             Intel(R) Core(TM) i5-5250U CPU @ 1.60GHz, 2 cores, 4
                    threads (x86_64)
  CPU features:     AVX: yes. AVX2: yes.
  Memory:           8.0 GB in total, 2.0 GB free
  Disk:             Unknown. Wrybill couldn't find the disk your home folder
                    is on.
  Graphics:         Unknown
  Network:          OK. There's a route to the internet. Nothing was sent to
                    test it.

Installed
  Shells:           fish (your login shell), sh, bash, zsh
  Package managers: None found
  git:              Not installed. It comes with Apple's developer tools.
                    Next: xcode-select --install
  Python:           Not installed. It comes with Apple's developer tools.
                    Next: xcode-select --install
  Node.js:          Not installed
  Java:             Not installed
  Docker:           Not installed
  Browsers:         None found. Wrybill's browser tool will need Chrome, Edge,
                    Brave, Chromium or Firefox.

The profile took 0.1 seconds.
"#;

const LINUX_SERVER: &str = r#"wrybill 0.1.0 (commit 3b22765f0a1b, aarch64-unknown-linux-musl)

Setup
  Config file:      Problem. $WRYBILL_HOME/config.toml is there, but Wrybill
                    can't read it (it isn't plain UTF-8 text).
                    Next: check that the file is plain text and that you're
                    allowed to read it.
  Keychain:         Warning. The Secret Service keyring can't be reached from
                    this session.
                    Next: on a server, in a container or over SSH, keep each
                    key in an environment variable and point the config at it,
                    like this:
                    api_key = "env:ANTHROPIC_API_KEY"
  Keys:             Not checked, because the config file can't be used.
  Logs:             3 files, 20 KB, in $WRYBILL_HOME/logs

This computer
  OS:               Ubuntu 24.04 (Linux kernel 6.8.0-45-generic)
  Security updates: Unknown. Wrybill's table only covers macOS and Windows, so
                    check with whoever makes this OS.
  Chip:             Unknown chip, 1 core (aarch64)
  CPU features:     None of the ones Wrybill looks for.
  Memory:           1.0 GB in total, 0.5 GB free
  Disk:             8.0 GB in total, 2.1 GB free (the disk your home folder is
                    on)
  Graphics:         Unknown
  Network:          OK. There's a route to the internet. Nothing was sent to
                    test it.

Installed
  Shells:           sh, bash (your login shell), dash
  Package managers: apt
  git:              2.43.0
  Python:           3.12.3
  Node.js:          Not installed
  Java:             Not installed
  Docker:           27.5.1
  Browsers:         None found. Wrybill's browser tool will need Chrome, Edge,
                    Brave, Chromium or Firefox.

The profile took 0.6 seconds.
"#;

/// The text with its line breaks and indents taken out, so a whole sentence
/// can be looked for wherever the layout happened to wrap it.
fn flowed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn every_layout() -> [(Report, &'static str); 4] {
    [
        (healthy_mac(), HEALTHY_MAC),
        (old_windows_laptop(), OLD_WINDOWS_LAPTOP),
        (bare_old_mac(), BARE_OLD_MAC),
        (linux_server(), LINUX_SERVER),
    ]
}

#[test]
fn a_healthy_mac_is_laid_out_like_this() {
    assert_eq!(render(&healthy_mac()), HEALTHY_MAC);
}

#[test]
fn an_old_windows_laptop_with_things_to_fix_is_laid_out_like_this() {
    assert_eq!(render(&old_windows_laptop()), OLD_WINDOWS_LAPTOP);
}

#[test]
fn an_old_mac_with_a_broken_config_is_laid_out_like_this() {
    assert_eq!(render(&bare_old_mac()), BARE_OLD_MAC);
}

#[test]
fn a_linux_server_with_no_keychain_is_laid_out_like_this() {
    assert_eq!(render(&linux_server()), LINUX_SERVER);
}

#[test]
fn the_output_is_plain_text_that_fits_an_old_console() {
    for (report, _) in every_layout() {
        let text = render(&report);

        // No colour and no symbols: nothing but plain ASCII.
        assert!(text.is_ascii());
        assert!(!text.contains('\u{1b}'), "there's a colour code in it");
        assert!(!text.contains('\t'));
        for line in text.lines() {
            assert!(line.len() <= 78, "{} characters: {line}", line.len());
            assert_eq!(line, line.trim_end(), "spaces at the end of a line");
        }
    }
}

#[test]
fn doctor_exits_with_1_only_when_the_config_cannot_be_used() {
    // Missing keys, an OS out of support and no network are all worth
    // saying, but they don't change the exit status.
    assert_eq!(healthy_mac().exit_status(), USABLE);
    assert_eq!(old_windows_laptop().exit_status(), USABLE);
    assert_eq!(bare_old_mac().exit_status(), NOT_USABLE);
    assert_eq!(linux_server().exit_status(), NOT_USABLE);
}

#[test]
fn the_other_answers_about_security_updates_read_like_this() {
    let with = |updates| {
        let mut report = healthy_mac();
        report.profile.security_updates = updates;
        flowed(&render(&report))
    };

    assert!(
        with(SecurityUpdates::Supported {
            until: Some(Date::new(2027, 10, 12))
        })
        .contains("OK. This version gets security updates until 12 October 2027.")
    );
    assert!(
        with(SecurityUpdates::OutOfSupport {
            since: Date::new(2021, 5, 11),
            or_earlier: true
        })
        .contains("Warning. Out of support: no security updates since 11 May 2021 or earlier.")
    );
    assert!(
        with(SecurityUpdates::Unknown(WhyUnknown::NotInTable))
            .contains("Unknown. This version or edition isn't in Wrybill's table.")
    );
    assert!(
        with(SecurityUpdates::Unknown(WhyUnknown::TableTooOld))
            .contains("Unknown. Wrybill's table is more than a year old")
    );
}

// The setup checks, against real folders and a keychain in memory.

/// A home folder for one test, inside Cargo's own temp folder. It doesn't
/// exist yet, and the real home folder is never touched.
fn home_for(test: &str) -> PathBuf {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("doctor")
        .join(test)
        .join("home");
    // Left over from an earlier run, if it's there at all.
    let _ = fs::remove_dir_all(&home);
    home
}

/// Wrybill's paths for someone whose home folder is `home`.
fn paths_in(home: &Path) -> Result<Paths, PathsError> {
    Paths::resolve(None, Some(home.to_path_buf()))
}

fn write_config(home: &Path, text: &[u8]) {
    let data_folder = home.join(".wrybill");
    fs::create_dir_all(&data_folder).expect("the data folder");
    fs::write(data_folder.join("config.toml"), text).expect("the config file");
}

/// `~/.wrybill/<name>`, with the separator of the OS the tests run on.
fn in_home(name: &str) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    format!("~{separator}.wrybill{separator}{name}")
}

fn no_variables(_: &str) -> Option<OsString> {
    None
}

/// The report doctor would give, with this setup in place of a real one.
fn report_with(setup: Setup) -> Report {
    Report {
        setup,
        ..healthy_mac()
    }
}

const THREE_MODELS_AND_SEARCH: &str = r#"version = 1

[search]
provider = "brave"

[[models]]
id = "claude-main"
provider = "anthropic"
model = "some-model"

[[models]]
id = "claude-fast"
provider = "anthropic"
model = "another-model"

[[models]]
id = "gpt"
provider = "openai"
model = "gpt-something"
api_key = "env:OPENAI_API_KEY"

[[models]]
id = "switched-off"
provider = "gemini"
model = "gemini-something"
enabled = false
"#;

#[test]
fn with_no_config_file_the_built_in_defaults_are_in_use() {
    let home = home_for("no-config");

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);

    assert_eq!(
        setup,
        Setup {
            config: ConfigCheck::Defaults {
                file: in_home("config.toml"),
            },
            keychain: KeychainCheck::Reachable("the test store"),
            keys: KeysCheck::Checked(Vec::new()),
            logs: LogsCheck::Empty {
                folder: in_home("logs"),
            },
        }
    );
    assert_eq!(report_with(setup).exit_status(), USABLE);
    // Checking the setup creates nothing.
    assert!(!home.exists());
}

#[test]
fn every_key_reference_in_use_is_looked_up_and_no_key_is_shown() {
    let home = home_for("key-references");
    write_config(&home, THREE_MODELS_AND_SEARCH.as_bytes());
    let store = MemoryStore::new();
    store
        .save("anthropic", &Secret::new(MARKER))
        .expect("the test store takes a key");
    let variables = |name: &str| (name == "OPENAI_API_KEY").then(|| OsString::from(MARKER));

    let setup = check_setup(&paths_in(&home), &store, &variables);

    assert_eq!(
        setup.config,
        ConfigCheck::Valid {
            file: in_home("config.toml"),
            warnings: Vec::new(),
        }
    );
    // The two Anthropic models share the key saved under the provider's
    // name. The model that's switched off isn't counted.
    assert_eq!(
        setup.keys,
        KeysCheck::Checked(vec![
            KeyLine {
                reference: keychain_key("anthropic"),
                models: 2,
                search: false,
                status: KeyStatus::Found,
            },
            KeyLine {
                reference: env_key("OPENAI_API_KEY"),
                models: 1,
                search: false,
                status: KeyStatus::Found,
            },
            KeyLine {
                reference: keychain_key("brave"),
                models: 0,
                search: true,
                status: KeyStatus::Missing,
            },
        ])
    );

    let text = render(&report_with(setup));
    assert!(text.contains("keychain:wrybill/anthropic: OK, used by 2 models."));
    assert!(text.contains("env:OPENAI_API_KEY: OK, used by 1 model."));
    assert!(text.contains("Warning: this key is kept in an environment variable."));
    assert!(text.contains("keychain:wrybill/brave: Missing, used by web search."));
    assert!(text.contains("Next: wrybill keys set brave"));
    assert!(!text.contains("MARKER"), "a key is in the output:\n{text}");
    assert!(!text.contains("gemini"), "{text}");
}

#[test]
fn a_key_used_by_a_model_and_by_search_is_one_line() {
    let home = home_for("shared-key");
    write_config(
        &home,
        b"version = 1\n\n[search]\nprovider = \"brave\"\napi_key = \"keychain:wrybill/shared\"\n\n\
          [[models]]\nid = \"m\"\nprovider = \"anthropic\"\nmodel = \"x\"\napi_key = \"keychain:wrybill/shared\"\n",
    );

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);

    assert_eq!(
        setup.keys,
        KeysCheck::Checked(vec![KeyLine {
            reference: keychain_key("shared"),
            models: 1,
            search: true,
            status: KeyStatus::Missing,
        }])
    );
    assert!(
        flowed(&render(&report_with(setup)))
            .contains("keychain:wrybill/shared: Missing, used by 1 model and web search.")
    );
}

#[test]
fn an_invalid_config_is_reported_without_showing_a_key_pasted_into_it() {
    let home = home_for("invalid-config");
    let config = format!(
        "version = 1\n[[models]]\nid = \"claude\"\nprovider = \"anthropic\"\nmodel = \"x\"\napi_key = \"{MARKER}\"\n"
    );
    write_config(&home, config.as_bytes());

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);

    let ConfigCheck::Invalid { errors, .. } = &setup.config else {
        panic!("expected an invalid config, but found {:?}", setup.config);
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.first().and_then(|error| error.line), Some(6));
    // With no config to go by, there are no references to check.
    assert_eq!(setup.keys, KeysCheck::NotChecked);

    let report = report_with(setup);
    let text = render(&report);
    assert_eq!(report.exit_status(), NOT_USABLE);
    assert!(
        flowed(&text).contains("has 1 error, so Wrybill can't use it:"),
        "{text}"
    );
    assert!(text.contains("Next: fix it in the file"), "{text}");
    // The profile is still printed.
    assert!(text.contains("This computer"), "{text}");
    assert!(!text.contains("MARKER"), "a key is in the output:\n{text}");
}

#[test]
fn a_config_that_is_not_text_is_reported_as_unreadable() {
    let home = home_for("unreadable-config");
    write_config(&home, &[0xff, 0xfe, 0x00, 0x41]);

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);

    assert_eq!(
        setup.config,
        ConfigCheck::Unreadable {
            file: in_home("config.toml"),
            reason: "it isn't plain UTF-8 text".to_owned(),
        }
    );
    assert_eq!(setup.keys, KeysCheck::NotChecked);
    assert_eq!(report_with(setup).exit_status(), NOT_USABLE);
}

#[test]
fn without_a_data_folder_doctor_says_why_and_still_prints_the_profile() {
    let setup = check_setup(
        &Err(PathsError::HomeNotAbsolute),
        &MemoryStore::new(),
        &no_variables,
    );

    assert_eq!(
        setup.config,
        ConfigCheck::NoDataFolder(PathsError::HomeNotAbsolute)
    );
    assert_eq!(setup.keys, KeysCheck::NotChecked);
    assert_eq!(setup.logs, LogsCheck::NoDataFolder);

    let report = report_with(setup);
    let text = render(&report);
    assert_eq!(report.exit_status(), NOT_USABLE);
    assert!(
        text.contains("Problem. WRYBILL_HOME must be the full path of a folder."),
        "{text}"
    );
    assert!(text.contains("None. There's no data folder to keep them in."));
    assert!(text.contains("This computer"));
}

#[test]
fn a_keychain_that_cannot_be_reached_is_a_warning_with_a_way_forward() {
    let home = home_for("no-keychain");
    write_config(&home, THREE_MODELS_AND_SEARCH.as_bytes());
    let store = MemoryStore::unreachable();

    let setup = check_setup(&paths_in(&home), &store, &no_variables);

    assert_eq!(setup.keychain, KeychainCheck::Unreachable("the test store"));
    let KeysCheck::Checked(lines) = &setup.keys else {
        panic!("expected the references to be listed");
    };
    assert!(matches!(
        lines.first().map(|line| &line.status),
        Some(KeyStatus::Unavailable(StoreError::Unreachable(_)))
    ));
    // A key in an environment variable needs no keychain.
    assert_eq!(
        lines.get(1).map(|line| &line.status),
        Some(&KeyStatus::Missing)
    );

    let report = report_with(setup);
    let text = render(&report);
    // It's a warning, not a reason to fail: the config itself is fine.
    assert_eq!(report.exit_status(), USABLE);
    let flowed_text = flowed(&text);
    assert!(
        flowed_text.contains("Warning. The test store can't be reached from this session."),
        "{text}"
    );
    assert!(text.contains("api_key = \"env:ANTHROPIC_API_KEY\""));
    assert!(
        flowed_text.contains(
            "keychain:wrybill/anthropic: Not checked, because the keychain can't be used. It's used by 2 models."
        ),
        "{text}"
    );
    // What the keychain said isn't printed. It only goes in the log.
    assert!(!text.contains("this test store has no keychain"), "{text}");
}

#[test]
fn the_log_files_are_counted_and_their_sizes_added_up() {
    let home = home_for("logs");
    let logs = home.join(".wrybill").join("logs");
    fs::create_dir_all(logs.join("a-folder")).expect("the logs folder");
    fs::write(logs.join("wrybill-2026-10-05.jsonl"), vec![b'x'; 1000]).expect("a log file");
    fs::write(logs.join("wrybill-2026-10-06.jsonl"), vec![b'x'; 500]).expect("a log file");

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);

    assert_eq!(
        setup.logs,
        LogsCheck::Kept {
            folder: in_home("logs"),
            files: 2,
            bytes: 1500,
        }
    );
    assert!(render(&report_with(setup)).contains("2 files, 2 KB, in ~"));
}

#[test]
fn a_data_folder_that_was_moved_is_shown_by_its_variable_wherever_it_is() {
    let home = home_for("moved-data-folder");
    let expected = if cfg!(windows) {
        r"%WRYBILL_HOME%\config.toml"
    } else {
        "$WRYBILL_HOME/config.toml"
    };

    // Moved out of the home folder, and moved to somewhere inside it. A
    // folder's name can say a lot, so neither path is shown.
    for moved_to in [
        home_for("moved-data-folder-elsewhere"),
        home.join("projects").join("a-secret-plan").join("wrybill"),
    ] {
        let paths = Paths::resolve(Some(moved_to.into_os_string()), Some(home.clone()));

        let setup = check_setup(&paths, &MemoryStore::new(), &no_variables);

        assert_eq!(
            setup.config,
            ConfigCheck::Defaults {
                file: expected.to_owned()
            }
        );
        assert!(!render(&report_with(setup)).contains("a-secret-plan"));
    }
}

#[test]
fn no_path_on_this_computer_is_ever_printed() {
    let home = home_for("no-paths");
    write_config(&home, THREE_MODELS_AND_SEARCH.as_bytes());

    let setup = check_setup(&paths_in(&home), &MemoryStore::new(), &no_variables);
    let text = render(&report_with(setup));

    // The home folder is shown as `~`, so nothing gives away where it is.
    let home_text = home.to_string_lossy().into_owned();
    assert!(
        !text.contains(&home_text),
        "the home folder is in the output"
    );
    assert!(text.contains(&in_home("config.toml")), "{text}");
}
