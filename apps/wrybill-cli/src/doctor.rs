//! `wrybill doctor`: prints the system profile and checks the setup
//! (spec 7.5 and 10.3).
//!
//! The output is plain text with no colour and no symbols, so it reads well
//! in an old Windows console and in a pasted issue. It leaves out the
//! computer's name, the user's name, serial numbers and network addresses,
//! and it never shows a key, so it's safe to paste somewhere public.

use std::ffi::OsString;
use std::io::Write;
use std::time::{Duration, Instant};

use wrybill_config::{
    Config, KeyRef, KeyStatus, LoadError, Loaded, Paths, PathsError, Problem, SecretStore, Source,
    StoreError, key_status, load,
};
use wrybill_tools::profile::{
    self, Chip, CpuFeatures, Disk, DiskKind, Graphics, Memory, Network, Os, OsFamily, Profile,
    Runtime, RuntimeState, SecurityUpdates, Shells, TABLE_CHECKED, WhyUnknown,
};

/// The exit status when the config can be used.
pub const USABLE: u8 = 0;

/// The exit status when the config is invalid, or can't be found or read.
pub const NOT_USABLE: u8 = 1;

/// How wide the output is kept, so it fits an 80-column console.
const WIDTH: usize = 78;

/// How far in each item starts.
const INDENT: usize = 2;

/// The room an item's label gets, with the space after it.
const LABEL_WIDTH: usize = 18;

/// The config file's name inside the data folder.
const CONFIG_FILE_NAME: &str = "config.toml";

/// The folder for Wrybill's own logs, inside the data folder.
const LOGS_FOLDER_NAME: &str = "logs";

/// Everything `wrybill doctor` reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Which build of Wrybill this is.
    pub build: Build,
    /// What was found out about Wrybill's own setup.
    pub setup: Setup,
    /// The system profile.
    pub profile: Profile,
}

impl Report {
    /// The exit status this report ends with: 1 when the config is invalid
    /// or can't be found or read, and 0 otherwise.
    pub fn exit_status(&self) -> u8 {
        match self.setup.config {
            ConfigCheck::Defaults { .. } | ConfigCheck::Valid { .. } => USABLE,
            ConfigCheck::NoDataFolder(_)
            | ConfigCheck::Invalid { .. }
            | ConfigCheck::Unreadable { .. } => NOT_USABLE,
        }
    }
}

/// Which build of Wrybill this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    /// The version, such as `0.1.0`.
    pub version: &'static str,
    /// The commit it was built from. CI sets it, and a build made by hand
    /// has none.
    pub commit: Option<&'static str>,
    /// The target it was built for, such as `x86_64-unknown-linux-musl`.
    pub target: &'static str,
}

impl Build {
    /// The build that is running.
    pub fn this_one() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            commit: option_env!("WRYBILL_BUILD_COMMIT"),
            target: env!("WRYBILL_BUILD_TARGET"),
        }
    }

    /// The line that names the build, which doctor prints first.
    pub fn line(&self) -> String {
        let commit = match self.commit.and_then(short_commit) {
            Some(commit) => format!("commit {commit}"),
            None => "a local build".to_owned(),
        };
        format!("wrybill {} ({commit}, {})", self.version, self.target)
    }
}

/// The start of a commit's name, if that's what the text is.
fn short_commit(commit: &str) -> Option<&str> {
    let commit = commit.trim();
    let is_a_commit = commit.len() >= 7 && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
    is_a_commit.then(|| commit.get(..12).unwrap_or(commit))
}

/// What was found out about Wrybill's own setup.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    /// The config file.
    pub config: ConfigCheck,
    /// The OS keychain.
    pub keychain: KeychainCheck,
    /// The keys the config points at.
    pub keys: KeysCheck,
    /// Wrybill's own logs.
    pub logs: LogsCheck,
}

/// What was found out about the config file.
///
/// A `file` is the config file's path as it's shown, with `~` for the home
/// folder.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigCheck {
    /// Wrybill can't work out where its data folder is.
    NoDataFolder(PathsError),
    /// There's no config file, so the built-in defaults are in use.
    Defaults {
        /// Where the file would be.
        file: String,
    },
    /// The file is valid.
    Valid {
        /// Where the file is.
        file: String,
        /// Things in it that are allowed but worth a second look.
        warnings: Vec<Problem>,
    },
    /// The file has errors, so nothing in it is used.
    Invalid {
        /// Where the file is.
        file: String,
        /// Everything that's wrong.
        errors: Vec<Problem>,
        /// Things in it that are allowed but worth a second look.
        warnings: Vec<Problem>,
    },
    /// The file is there, but it can't be read.
    Unreadable {
        /// Where the file is.
        file: String,
        /// Why not.
        reason: String,
    },
}

/// Whether the OS keychain can be used. Each holds the keychain's name, such
/// as `the macOS Keychain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainCheck {
    /// It can.
    Reachable(&'static str),
    /// It can't be reached from this session.
    Unreachable(&'static str),
    /// It was reached, and reported a problem.
    Failed(&'static str),
}

/// What was found out about the keys the config points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeysCheck {
    /// The config can't be used, so there are no references to check.
    NotChecked,
    /// Each reference in the config, and what it points at.
    Checked(Vec<KeyLine>),
}

/// One key reference from the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLine {
    /// The reference. It names where a key is kept, and is never the key.
    pub reference: KeyRef,
    /// How many models that are switched on use it.
    pub models: usize,
    /// Whether web search uses it.
    pub search: bool,
    /// Whether there's a key where it points.
    pub status: KeyStatus,
}

/// What was found out about Wrybill's own logs.
///
/// A `folder` is the logs folder's path as it's shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogsCheck {
    /// There's no data folder to keep them in.
    NoDataFolder,
    /// There are none yet.
    Empty {
        /// Where they would be.
        folder: String,
    },
    /// The log files and how much disk they use.
    Kept {
        /// Where they are.
        folder: String,
        /// How many files there are.
        files: usize,
        /// Their size in bytes, added up.
        bytes: u64,
    },
}

/// Runs `wrybill doctor`: checks the setup, works out the profile and prints
/// both to `out`. The exit status comes back.
pub fn run(store: &dyn SecretStore, out: &mut dyn Write) -> u8 {
    let started = Instant::now();
    tracing::info!(command = "doctor", "started");

    let paths = Paths::from_env();
    let setup = check_setup(&paths, store, &|name| std::env::var_os(name));
    let report = Report {
        build: Build::this_one(),
        setup,
        profile: profile::collect(),
    };
    let status = report.exit_status();

    // If the terminal has gone away there's nobody to tell, so a failed
    // write is let go.
    let _ = out.write_all(render(&report).as_bytes());

    tracing::info!(
        command = "doctor",
        exit_status = status,
        took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "finished"
    );
    status
}

/// Checks Wrybill's own setup: the config file, the keychain, the keys the
/// config points at and the logs.
///
/// `paths` is where Wrybill keeps its data, or why that can't be worked out.
/// `env_var` looks up an environment variable. Both are handed in so tests
/// never touch the real environment.
pub fn check_setup(
    paths: &Result<Paths, PathsError>,
    store: &dyn SecretStore,
    env_var: &dyn Fn(&str) -> Option<OsString>,
) -> Setup {
    let keychain = match store.check() {
        Ok(()) => KeychainCheck::Reachable(store.name()),
        Err(error) => {
            // What the OS said goes in the log, which stays on this
            // computer. It isn't printed, in case it names something.
            tracing::warn!(
                command = "doctor",
                detail = %error,
                "the keychain can't be used"
            );
            match error {
                StoreError::Unreachable(_) => KeychainCheck::Unreachable(store.name()),
                StoreError::Failed(_) | StoreError::BadName => KeychainCheck::Failed(store.name()),
            }
        }
    };

    let (config, keys, logs) = match paths {
        Err(error) => (
            ConfigCheck::NoDataFolder(*error),
            KeysCheck::NotChecked,
            LogsCheck::NoDataFolder,
        ),
        Ok(paths) => {
            let file = shown_in_data_folder(paths, CONFIG_FILE_NAME);
            let (config, keys) = match load(&paths.config_file(), paths.user_home()) {
                Ok(Loaded {
                    config,
                    source: Source::Defaults,
                    ..
                }) => (
                    ConfigCheck::Defaults { file },
                    KeysCheck::Checked(key_lines(&config, store, env_var)),
                ),
                Ok(Loaded {
                    config,
                    source: Source::File,
                    warnings,
                }) => (
                    ConfigCheck::Valid { file, warnings },
                    KeysCheck::Checked(key_lines(&config, store, env_var)),
                ),
                Err(LoadError::Invalid(invalid)) => (
                    ConfigCheck::Invalid {
                        file,
                        errors: invalid.errors,
                        warnings: invalid.warnings,
                    },
                    KeysCheck::NotChecked,
                ),
                Err(LoadError::Unreadable(reason)) => (
                    ConfigCheck::Unreadable { file, reason },
                    KeysCheck::NotChecked,
                ),
            };
            (config, keys, check_logs(paths))
        }
    };

    // Counts only. A config's own words never go in the log.
    let (state, errors, warnings) = match &config {
        ConfigCheck::NoDataFolder(_) => ("no data folder", 0, 0),
        ConfigCheck::Defaults { .. } => ("defaults", 0, 0),
        ConfigCheck::Valid { warnings, .. } => ("valid", 0, warnings.len()),
        ConfigCheck::Invalid {
            errors, warnings, ..
        } => ("invalid", errors.len(), warnings.len()),
        ConfigCheck::Unreadable { .. } => ("unreadable", 0, 0),
    };
    tracing::info!(
        command = "doctor",
        config = state,
        errors,
        warnings,
        "checked the config"
    );

    Setup {
        config,
        keychain,
        keys,
        logs,
    }
}

/// Every key reference that a switched-on model or web search uses, in the
/// order they come up, with what each points at. No key is read.
fn key_lines(
    config: &Config,
    store: &dyn SecretStore,
    env_var: &dyn Fn(&str) -> Option<OsString>,
) -> Vec<KeyLine> {
    // Each use of a reference, and whether it's web search that uses it.
    let models = config
        .models
        .iter()
        .filter(|model| model.enabled)
        .filter_map(|model| model.api_key.as_ref())
        .map(|reference| (reference, false));
    let search = config
        .search
        .iter()
        .filter_map(|search| search.api_key.as_ref())
        .map(|reference| (reference, true));

    let mut lines: Vec<KeyLine> = Vec::new();
    for (reference, for_search) in models.chain(search) {
        let index = match lines.iter().position(|line| &line.reference == reference) {
            Some(index) => index,
            None => {
                lines.push(KeyLine {
                    reference: reference.clone(),
                    models: 0,
                    search: false,
                    status: key_status(reference, store, env_var),
                });
                lines.len() - 1
            }
        };
        if let Some(line) = lines.get_mut(index) {
            if for_search {
                line.search = true;
            } else {
                line.models += 1;
            }
        }
    }
    lines
}

/// Counts the log files and adds up their size.
fn check_logs(paths: &Paths) -> LogsCheck {
    let folder = shown_in_data_folder(paths, LOGS_FOLDER_NAME);
    let Ok(entries) = std::fs::read_dir(paths.logs_folder()) else {
        return LogsCheck::Empty { folder };
    };

    let (mut files, mut bytes) = (0_usize, 0_u64);
    for about in entries.flatten().filter_map(|entry| entry.metadata().ok()) {
        if about.is_file() {
            files += 1;
            bytes = bytes.saturating_add(about.len());
        }
    }
    if files == 0 {
        LogsCheck::Empty { folder }
    } else {
        LogsCheck::Kept {
            folder,
            files,
            bytes,
        }
    }
}

/// How to show something inside the data folder without giving anything
/// away. In the usual place that's `~/.wrybill/<name>`, which is the same on
/// every computer. A data folder that was moved is shown through the
/// variable that moved it, never by the path it holds.
fn shown_in_data_folder(paths: &Paths, name: &str) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let usual_name = paths
        .data_folder()
        .file_name()
        .filter(|_| paths.data_folder_is_default());
    match usual_name {
        Some(folder) => format!("~{separator}{}{separator}{name}", folder.to_string_lossy()),
        None if cfg!(windows) => format!(r"%WRYBILL_HOME%\{name}"),
        None => format!("$WRYBILL_HOME/{name}"),
    }
}

/// Lays the report out as the text `wrybill doctor` prints.
pub fn render(report: &Report) -> String {
    let mut page = Page::default();
    page.line(&report.build.line());

    page.heading("Setup");
    page.item("Config file:", &config_lines(&report.setup.config));
    page.item("Keychain:", &keychain_lines(report.setup.keychain));
    page.item("Keys:", &keys_lines(&report.setup.keys));
    page.item("Logs:", &[logs_line(&report.setup.logs)]);

    let profile = &report.profile;
    page.heading("This computer");
    page.item("OS:", &[os_line(&profile.os)]);
    page.item(
        "Security updates:",
        &updates_lines(profile.security_updates, profile.os.family),
    );
    page.item("Chip:", &[chip_line(&profile.chip)]);
    page.item("CPU features:", &[features_line(&profile.chip.features)]);
    page.item("Memory:", &[memory_line(profile.memory)]);
    page.item("Disk:", &[disk_line(profile.disk)]);
    page.item("Graphics:", &[graphics_line(&profile.installed.graphics)]);
    page.item("Network:", &[network_line(profile.network)]);

    let installed = &profile.installed;
    page.heading("Installed");
    page.item("Shells:", &[shells_line(&installed.shells)]);
    page.item(
        "Package managers:",
        &[list_or_none(&installed.package_managers)],
    );
    for runtime in &installed.runtimes {
        page.item(&format!("{}:", runtime.name), &runtime_lines(runtime));
    }
    page.item("Browsers:", &[browsers_line(&installed.browsers)]);

    page.blank();
    page.line(&format!(
        "The profile took {} seconds.",
        seconds(profile.took)
    ));

    plain(&page.text)
}

/// The text being laid out.
#[derive(Default)]
struct Page {
    text: String,
}

impl Page {
    fn blank(&mut self) {
        self.text.push('\n');
    }

    fn line(&mut self, text: &str) {
        self.text.push_str(text);
        self.text.push('\n');
    }

    fn heading(&mut self, title: &str) {
        self.blank();
        self.line(title);
    }

    /// One labelled item. Each paragraph starts a new line under the first,
    /// and a long one is wrapped so it stays in its column.
    fn item(&mut self, label: &str, paragraphs: &[String]) {
        let room = WIDTH.saturating_sub(INDENT + LABEL_WIDTH);
        let mut label = Some(label);

        for paragraph in paragraphs {
            for line in wrapped_item(paragraph, room) {
                let shown_label = label.take().unwrap_or_default();
                self.text.push_str(&" ".repeat(INDENT));
                self.text
                    .push_str(&format!("{shown_label:<width$}{line}", width = LABEL_WIDTH));
                self.text.push('\n');
            }
        }
    }
}

/// Wraps one paragraph of an item. One that starts with `- ` is a point in a
/// list, so its later lines are set in under its words, not under the dash.
fn wrapped_item(paragraph: &str, room: usize) -> Vec<String> {
    let Some(point) = paragraph.strip_prefix("- ") else {
        return wrapped(paragraph, room);
    };
    wrapped(point, room.saturating_sub(2))
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let lead = if index == 0 { "- " } else { "  " };
            format!("{lead}{line}")
        })
        .collect()
}

/// Breaks text into lines of at most `room` characters, at spaces. A single
/// word that's longer, such as a path, is left whole.
fn wrapped(text: &str, room: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > room {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Makes sure nothing but plain ASCII is printed, whatever a path or the OS
/// put into a message. Anything else becomes a `?`.
fn plain(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c == '\n' || c == ' ' || c.is_ascii_graphic() {
                c
            } else {
                '?'
            }
        })
        .collect()
}

fn config_lines(config: &ConfigCheck) -> Vec<String> {
    match config {
        ConfigCheck::NoDataFolder(error) => vec![format!("Problem. {error}")],
        ConfigCheck::Defaults { file } => vec![format!(
            "OK. There's no {file}, so Wrybill is using its built-in defaults."
        )],
        ConfigCheck::Valid { file, warnings } => {
            let mut lines = vec![format!("OK. {file} is valid.")];
            lines.extend(warnings.iter().map(|warning| format!("Warning: {warning}")));
            lines
        }
        ConfigCheck::Invalid {
            file,
            errors,
            warnings,
        } => {
            let mut lines = vec![format!(
                "Problem. {file} has {}, so Wrybill can't use it:",
                counted(errors.len(), "error", "errors")
            )];
            lines.extend(errors.iter().map(|error| format!("- {error}")));
            lines.extend(warnings.iter().map(|warning| format!("Warning: {warning}")));
            lines.push(format!(
                "Next: fix {} in the file, then run wrybill doctor again.",
                if errors.len() == 1 { "it" } else { "them" }
            ));
            lines
        }
        ConfigCheck::Unreadable { file, reason } => vec![
            format!("Problem. {file} is there, but Wrybill can't read it ({reason})."),
            "Next: check that the file is plain text and that you're allowed to read it."
                .to_owned(),
        ],
    }
}

fn keychain_lines(keychain: KeychainCheck) -> Vec<String> {
    match keychain {
        KeychainCheck::Reachable(name) => {
            vec![format!("OK. {} can be reached.", capitalised(name))]
        }
        KeychainCheck::Unreachable(name) => vec![
            format!(
                "Warning. {} can't be reached from this session.",
                capitalised(name)
            ),
            "Next: on a server, in a container or over SSH, keep each key in an environment variable and point the config at it, like this:"
                .to_owned(),
            "api_key = \"env:ANTHROPIC_API_KEY\"".to_owned(),
        ],
        KeychainCheck::Failed(name) => vec![format!(
            "Warning. {} reported a problem. Today's log file has the details.",
            capitalised(name)
        )],
    }
}

fn keys_lines(keys: &KeysCheck) -> Vec<String> {
    let lines = match keys {
        KeysCheck::NotChecked => {
            return vec!["Not checked, because the config file can't be used.".to_owned()];
        }
        KeysCheck::Checked(lines) => lines,
    };
    if lines.is_empty() {
        return vec!["None needed. Nothing in the config uses a key yet.".to_owned()];
    }

    let mut shown = Vec::new();
    for line in lines {
        let reference = &line.reference;
        let used_by = used_by(line);
        match (&line.status, reference) {
            (KeyStatus::Found, _) => shown.push(format!("{reference}: OK, {used_by}.")),
            (KeyStatus::Missing, KeyRef::Keychain { name }) => {
                shown.push(format!("{reference}: Missing, {used_by}."));
                shown.push(format!("Next: wrybill keys set {name}"));
            }
            (KeyStatus::Missing, KeyRef::Env { variable }) => {
                shown.push(format!(
                    "{reference}: Missing, {used_by}. The variable isn't set, or it's empty."
                ));
                shown.push(format!(
                    "Next: set {variable} in the environment Wrybill runs in."
                ));
            }
            (KeyStatus::Unavailable(_), _) => shown.push(format!(
                "{reference}: Not checked, because the keychain can't be used. It's {used_by}."
            )),
        }
        if matches!(reference, KeyRef::Env { .. }) {
            shown.push(
                "Warning: this key is kept in an environment variable. That's meant for machines with no keychain to use."
                    .to_owned(),
            );
        }
    }
    shown
}

/// Who uses a key, such as `used by 2 models and web search`.
fn used_by(line: &KeyLine) -> String {
    let models = (line.models > 0).then(|| counted(line.models, "model", "models"));
    match (models, line.search) {
        (Some(models), true) => format!("used by {models} and web search"),
        (Some(models), false) => format!("used by {models}"),
        (None, _) => "used by web search".to_owned(),
    }
}

fn logs_line(logs: &LogsCheck) -> String {
    match logs {
        LogsCheck::NoDataFolder => "None. There's no data folder to keep them in.".to_owned(),
        LogsCheck::Empty { folder } => format!("None yet. They will go in {folder}"),
        LogsCheck::Kept {
            folder,
            files,
            bytes,
        } => format!(
            "{}, {}, in {folder}",
            counted(*files, "file", "files"),
            file_size(*bytes)
        ),
    }
}

fn os_line(os: &Os) -> String {
    let mut line = os.name.clone();
    match &os.version {
        // Windows gives a build number, which reads better in brackets.
        Some(version) if version.starts_with("build ") => {
            line.push_str(&format!(" ({version})"));
        }
        Some(version) => line.push_str(&format!(" {version}")),
        None => {}
    }
    if let Some(kernel) = &os.kernel {
        line.push_str(&format!(" (Linux kernel {kernel})"));
    }
    line
}

fn updates_lines(updates: SecurityUpdates, family: OsFamily) -> Vec<String> {
    let checked = format!("Wrybill's table of OS versions was checked on {TABLE_CHECKED}.");
    match updates {
        SecurityUpdates::Supported { until: None } => vec![
            "OK. This version still gets security updates.".to_owned(),
            checked,
        ],
        SecurityUpdates::Supported { until: Some(until) } => vec![
            format!("OK. This version gets security updates until {until}."),
            checked,
        ],
        SecurityUpdates::SupportedIf { condition, until } => {
            let until = until.map_or_else(String::new, |until| format!(", until {until}"));
            let mut lines = vec![format!(
                "Warning. This version gets security updates {condition}{until}."
            )];
            if family == OsFamily::Windows {
                lines.push(
                    "Next: open Settings, then Windows Update, and check that this PC is enrolled."
                        .to_owned(),
                );
            }
            lines.push(checked);
            lines
        }
        SecurityUpdates::OutOfSupport { since, or_earlier } => vec![
            format!(
                "Warning. Out of support: no security updates since {since}{}.",
                if or_earlier { " or earlier" } else { "" }
            ),
            "Next: update the OS if this computer can run a newer one. For an old laptop, a current Linux distribution is the safest home."
                .to_owned(),
            checked,
        ],
        SecurityUpdates::Unknown(WhyUnknown::OsNotCovered) => vec![
            "Unknown. Wrybill's table only covers macOS and Windows, so check with whoever makes this OS."
                .to_owned(),
        ],
        SecurityUpdates::Unknown(WhyUnknown::NotInTable) => vec![
            "Unknown. This version or edition isn't in Wrybill's table.".to_owned(),
            checked,
        ],
        SecurityUpdates::Unknown(WhyUnknown::TableTooOld) => vec![
            "Unknown. Wrybill's table is more than a year old, so it can't say. A newer Wrybill will know."
                .to_owned(),
            checked,
        ],
    }
}

fn chip_line(chip: &Chip) -> String {
    let name = chip.name.as_deref().unwrap_or("Unknown chip");
    let cores = match chip.physical_cores {
        Some(physical) if physical != chip.logical_cores => format!(
            "{}, {}",
            counted(physical, "core", "cores"),
            counted(chip.logical_cores, "thread", "threads")
        ),
        _ => counted(chip.logical_cores, "core", "cores"),
    };
    format!("{name}, {cores} ({})", chip.architecture)
}

fn features_line(features: &CpuFeatures) -> String {
    let others = features.others.join(", ");
    match (features.avx, features.avx2) {
        (Some(avx), Some(avx2)) => {
            let mut line = format!("AVX: {}. AVX2: {}.", yes_or_no(avx), yes_or_no(avx2));
            if !others.is_empty() {
                line.push_str(&format!(" Also: {others}."));
            }
            line
        }
        // AVX belongs to x86 chips, so on any other kind there's no answer.
        _ if others.is_empty() => "None of the ones Wrybill looks for.".to_owned(),
        _ => format!("{others}. AVX and AVX2 don't apply to this kind of chip."),
    }
}

fn memory_line(memory: Memory) -> String {
    format!(
        "{} in total, {} free",
        memory_size(memory.total_bytes),
        memory_size(memory.free_bytes)
    )
}

fn disk_line(disk: Option<Disk>) -> String {
    let Some(disk) = disk else {
        return "Unknown. Wrybill couldn't find the disk your home folder is on.".to_owned();
    };
    let kind = match disk.kind {
        DiskKind::Ssd => ", an SSD",
        DiskKind::Hdd => ", a spinning hard drive",
        DiskKind::Unknown => "",
    };
    format!(
        "{} in total, {} free{kind} (the disk your home folder is on)",
        disk_size(disk.total_bytes),
        disk_size(disk.free_bytes)
    )
}

fn graphics_line(graphics: &Graphics) -> String {
    match graphics {
        Graphics::Found(chips) => chips.join(", "),
        Graphics::Unknown => "Unknown".to_owned(),
    }
}

fn network_line(network: Network) -> String {
    match network {
        Network::Up => {
            "OK. There's a route to the internet. Nothing was sent to test it.".to_owned()
        }
        Network::NoRoute => {
            "Warning. There's no route to the internet. Wrybill sticks to local and LAN brains, and web tools stay off, until the connection is back."
                .to_owned()
        }
    }
}

fn shells_line(shells: &Shells) -> String {
    let login = shells.login.as_deref();
    let mut names: Vec<String> = Vec::new();
    // A login shell Wrybill doesn't look for is still worth naming.
    if let Some(login) = login
        && !shells.found.contains(&login)
    {
        names.push(format!("{login} (your login shell)"));
    }
    for name in &shells.found {
        names.push(if Some(*name) == login {
            format!("{name} (your login shell)")
        } else {
            (*name).to_owned()
        });
    }
    if names.is_empty() {
        "None found".to_owned()
    } else {
        names.join(", ")
    }
}

fn list_or_none(names: &[&'static str]) -> String {
    if names.is_empty() {
        "None found".to_owned()
    } else {
        names.join(", ")
    }
}

fn runtime_lines(runtime: &Runtime) -> Vec<String> {
    match &runtime.state {
        RuntimeState::Found {
            version: Some(version),
        } => vec![version.clone()],
        RuntimeState::Found { version: None } => {
            vec!["Installed, but Wrybill couldn't read its version.".to_owned()]
        }
        RuntimeState::NotInstalled => vec!["Not installed".to_owned()],
        RuntimeState::NeedsDeveloperTools => vec![
            "Not installed. It comes with Apple's developer tools.".to_owned(),
            "Next: xcode-select --install".to_owned(),
        ],
        RuntimeState::NoAnswer => {
            vec!["Warning. It's there, but it didn't answer in time.".to_owned()]
        }
    }
}

fn browsers_line(browsers: &[&'static str]) -> String {
    if browsers.is_empty() {
        "None found. Wrybill's browser tool will need Chrome, Edge, Brave, Chromium or Firefox."
            .to_owned()
    } else {
        browsers.join(", ")
    }
}

/// A count with its noun, such as `1 error` or `3 errors`.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn yes_or_no(yes: bool) -> &'static str {
    if yes { "yes" } else { "no" }
}

fn capitalised(text: &str) -> String {
    let mut letters = text.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// Memory, counted the way memory is sold: 8 GB is 8 times 1024 MB.
fn memory_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.1} GB", bytes as f64 / GB)
}

/// Disk space, counted the way disks are sold: 500 GB is 500 thousand
/// million bytes.
fn disk_size(bytes: u64) -> String {
    const GB: f64 = 1_000_000_000.0;
    let size = bytes as f64 / GB;
    if size < 10.0 {
        format!("{size:.1} GB")
    } else {
        format!("{size:.0} GB")
    }
}

fn file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if bytes < KB {
        counted(
            usize::try_from(bytes).unwrap_or(usize::MAX),
            "byte",
            "bytes",
        )
    } else if bytes < MB {
        format!("{} KB", bytes.div_ceil(KB))
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

/// A length of time in seconds, to one decimal place. Anything shorter than
/// a tenth of a second is shown as one.
fn seconds(took: Duration) -> String {
    format!("{:.1}", took.as_secs_f64().max(0.1))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        Build, disk_size, file_size, memory_size, plain, seconds, short_commit, wrapped,
        wrapped_item,
    };

    #[test]
    fn the_first_line_names_the_version_the_commit_and_the_target() {
        let from_ci = Build {
            version: "0.1.0",
            commit: Some("3b22765f0a1b2c3d4e5f60718293a4b5c6d7e8f9"),
            target: "x86_64-unknown-linux-musl",
        };
        let by_hand = Build {
            commit: None,
            target: "aarch64-apple-darwin",
            ..from_ci
        };

        assert_eq!(
            from_ci.line(),
            "wrybill 0.1.0 (commit 3b22765f0a1b, x86_64-unknown-linux-musl)"
        );
        assert_eq!(
            by_hand.line(),
            "wrybill 0.1.0 (a local build, aarch64-apple-darwin)"
        );
    }

    #[test]
    fn only_something_that_looks_like_a_commit_is_shown_as_one() {
        assert_eq!(short_commit("3b22765"), Some("3b22765"));
        assert_eq!(short_commit(" 3B22765F0A1B2C3D \n"), Some("3B22765F0A1B"));
        for not_a_commit in ["", "3b2276", "main", "3b22765-dirty", "not hexadecimal"] {
            assert_eq!(short_commit(not_a_commit), None, "{not_a_commit:?}");
        }
    }

    #[test]
    fn long_text_is_wrapped_at_spaces() {
        assert_eq!(
            wrapped("one two three four five", 9),
            ["one two", "three", "four five"]
        );
        assert_eq!(wrapped("fits", 9), ["fits"]);
        assert_eq!(wrapped("exactly 9", 9), ["exactly 9"]);
        // A word longer than the line is left whole.
        assert_eq!(
            wrapped("see ~/a/very/long/path/config.toml now", 9),
            ["see", "~/a/very/long/path/config.toml", "now"]
        );
        assert_eq!(wrapped("", 9), [""]);
    }

    #[test]
    fn a_point_in_a_list_keeps_its_later_lines_under_its_words() {
        assert_eq!(
            wrapped_item("- one two three four", 9),
            ["- one two", "  three", "  four"]
        );
        assert_eq!(wrapped_item("not a point", 9), ["not a", "point"]);
    }

    #[test]
    fn anything_that_is_not_plain_ascii_becomes_a_question_mark() {
        assert_eq!(plain("caf\u{e9}\n  ok\ttab"), "caf?\n  ok?tab");
    }

    #[test]
    fn sizes_are_shown_the_way_each_thing_is_sold() {
        assert_eq!(memory_size(8 * 1024 * 1024 * 1024), "8.0 GB");
        assert_eq!(memory_size(4_123_456_789), "3.8 GB");
        assert_eq!(disk_size(500_107_862_016), "500 GB");
        assert_eq!(disk_size(7_500_000_000), "7.5 GB");
        assert_eq!(file_size(0), "0 bytes");
        assert_eq!(file_size(1), "1 byte");
        assert_eq!(file_size(1023), "1023 bytes");
        assert_eq!(file_size(1024), "1 KB");
        assert_eq!(file_size(1025), "2 KB");
        assert_eq!(file_size(5 * 1024 * 1024 + 512 * 1024), "5.5 MB");
    }

    #[test]
    fn the_time_taken_is_shown_to_a_tenth_of_a_second() {
        assert_eq!(seconds(Duration::from_millis(289)), "0.3");
        assert_eq!(seconds(Duration::from_millis(4)), "0.1");
        assert_eq!(seconds(Duration::from_millis(2_449)), "2.4");
    }
}
