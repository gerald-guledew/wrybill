//! What's installed: shells, package managers, runtimes and browsers
//! (spec 10.3).
//!
//! A program is only run when its answer is needed, which means a runtime's
//! version. Shells and package managers are found by looking along the
//! `PATH`, and browsers by looking where each OS puts them. A browser is
//! never started.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::graphics::{self, Graphics};
use super::path_search::SearchPath;
use super::run::{self, Job, Ran, RunError};
use super::{OsFamily, plain_if_any};

/// How many programs are run at once. Small, because an old laptop has two
/// cores and a slow disk.
const AT_ONCE: usize = 4;

/// How long a program may take to say its version.
const VERSION_LIMIT: Duration = Duration::from_secs(5);

/// The longest version that's kept.
const MOST_VERSION: usize = 32;

/// How many lines of a program's output are searched for its version.
const LINES_SEARCHED: usize = 10;

/// What's installed on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The shells.
    pub shells: Shells,
    /// The package managers that were found, by name.
    pub package_managers: Vec<&'static str>,
    /// The runtimes Wrybill looks for, found or not, always in the same
    /// order: git, Python, Node.js, Java and Docker.
    pub runtimes: Vec<Runtime>,
    /// The browsers that were found, by name.
    pub browsers: Vec<&'static str>,
    /// The graphics chips.
    pub graphics: Graphics,
}

/// The shells on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shells {
    /// The user's login shell, on macOS and Linux.
    pub login: Option<String>,
    /// The shells that were found, by name.
    pub found: Vec<&'static str>,
}

/// A runtime Wrybill looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    /// Its name, such as `Python`.
    pub name: &'static str,
    /// Whether it's there.
    pub state: RuntimeState,
}

/// Whether a runtime is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeState {
    /// It's installed.
    Found {
        /// Its version, when the program's answer could be read.
        version: Option<String>,
    },
    /// It isn't installed.
    NotInstalled,
    /// On a Mac, only Apple's stand-in is there, and it does nothing until
    /// Apple's developer tools are installed.
    NeedsDeveloperTools,
    /// It's there, but it didn't answer in time.
    NoAnswer,
}

/// A runtime to look for.
struct Wanted {
    name: &'static str,
    /// The programs that could be it, first choice first.
    programs: &'static [&'static str],
    /// What to pass a program to ask for its version. Always something: with
    /// no argument, a stand-in on Windows opens the Microsoft Store.
    args: &'static [&'static str],
}

const RUNTIMES: [Wanted; 5] = [
    Wanted {
        name: "git",
        programs: &["git"],
        args: &["--version"],
    },
    Wanted {
        name: "Python",
        programs: &["python3", "python"],
        args: &["--version"],
    },
    Wanted {
        name: "Node.js",
        programs: &["node"],
        args: &["--version"],
    },
    Wanted {
        name: "Java",
        programs: &["java"],
        args: &["-version"],
    },
    // The command-line program only. The Docker daemon isn't contacted, and
    // `--version` doesn't need it.
    Wanted {
        name: "Docker",
        programs: &["docker"],
        args: &["--version"],
    },
];

/// The Python launcher that Windows installs of Python come with.
const WINDOWS_PYTHON_LAUNCHER: &str = "py";

/// Shells on macOS and Linux: the name to show, and the program.
const UNIX_SHELLS: [(&str, &str); 7] = [
    ("sh", "sh"),
    ("bash", "bash"),
    ("zsh", "zsh"),
    ("fish", "fish"),
    ("dash", "dash"),
    ("ksh", "ksh"),
    ("PowerShell", "pwsh"),
];

/// Shells on Windows: the name to show, and the program.
const WINDOWS_SHELLS: [(&str, &str); 4] = [
    ("cmd", "cmd"),
    ("Windows PowerShell", "powershell"),
    ("PowerShell", "pwsh"),
    ("bash", "bash"),
];

/// Package managers: the name to show, and the program.
const PACKAGE_MANAGERS: [(&str, &str); 6] = [
    ("Homebrew", "brew"),
    ("winget", "winget"),
    ("Scoop", "scoop"),
    ("apt", "apt-get"),
    ("dnf", "dnf"),
    ("pacman", "pacman"),
];

/// A browser Wrybill can drive (spec 10.4), and where each OS puts it.
struct Browser {
    name: &'static str,
    /// macOS: the app's name in an Applications folder.
    mac_app: &'static str,
    /// Windows: the program, below one of the folders programs install to.
    windows_program: &'static str,
    /// Linux: the commands it goes by.
    linux_commands: &'static [&'static str],
    /// Linux: its name as a Flatpak.
    flatpak: &'static str,
}

const BROWSERS: [Browser; 5] = [
    Browser {
        name: "Chrome",
        mac_app: "Google Chrome.app",
        windows_program: r"Google\Chrome\Application\chrome.exe",
        linux_commands: &["google-chrome", "google-chrome-stable"],
        flatpak: "com.google.Chrome",
    },
    Browser {
        name: "Edge",
        mac_app: "Microsoft Edge.app",
        windows_program: r"Microsoft\Edge\Application\msedge.exe",
        linux_commands: &["microsoft-edge", "microsoft-edge-stable"],
        flatpak: "com.microsoft.Edge",
    },
    Browser {
        name: "Brave",
        mac_app: "Brave Browser.app",
        windows_program: r"BraveSoftware\Brave-Browser\Application\brave.exe",
        linux_commands: &["brave-browser", "brave"],
        flatpak: "com.brave.Browser",
    },
    Browser {
        name: "Chromium",
        mac_app: "Chromium.app",
        windows_program: r"Chromium\Application\chrome.exe",
        linux_commands: &["chromium", "chromium-browser"],
        flatpak: "org.chromium.Chromium",
    },
    Browser {
        name: "Firefox",
        mac_app: "Firefox.app",
        windows_program: r"Mozilla Firefox\firefox.exe",
        linux_commands: &["firefox", "firefox-esr"],
        flatpak: "org.mozilla.firefox",
    },
];

/// The folders browsers are looked for in. Which ones matter depends on the
/// OS.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BrowserPlaces {
    /// macOS: the Applications folders.
    applications: Vec<PathBuf>,
    /// Windows: the folders programs install to.
    program_folders: Vec<PathBuf>,
    /// Linux: the folders Flatpak puts its launchers in.
    flatpak_folders: Vec<PathBuf>,
}

impl BrowserPlaces {
    /// The real places on this computer.
    fn on_this_computer(home: Option<&Path>) -> Self {
        let from_env = |name: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|folder| folder.is_absolute())
        };

        let mut applications = vec![PathBuf::from("/Applications")];
        applications.extend(home.map(|home| home.join("Applications")));

        let program_folders = ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"]
            .into_iter()
            .filter_map(from_env)
            .collect();

        let mut flatpak_folders = vec![PathBuf::from("/var/lib/flatpak/exports/bin")];
        flatpak_folders.extend(home.map(|home| home.join(".local/share/flatpak/exports/bin")));

        Self {
            applications,
            program_folders,
            flatpak_folders,
        }
    }
}

/// What's known about Apple's stand-in programs on a Mac.
///
/// `/usr/bin/git`, `/usr/bin/python3` and `/usr/bin/java` are there on every
/// Mac, whether or not anything is behind them. Running one with nothing
/// behind it can put a dialog on the screen, so that's never done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AppleFacts {
    /// Whether Apple's developer tools are installed, which are what's
    /// behind `git` and `python3`.
    developer_tools: bool,
    /// Whether any Java is installed, which is what's behind `java`.
    java: bool,
}

/// What to do with a program that was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Run it and read its answer.
    Run,
    /// Don't run it: it's Apple's stand-in for the developer tools, and
    /// they aren't installed.
    NeedsDeveloperTools,
    /// Don't run it: it's Apple's stand-in for Java, and no Java is
    /// installed.
    NothingBehindIt,
}

/// Finds out what's installed.
pub(super) fn collect(family: OsFamily) -> Installed {
    let search = SearchPath::from_env();
    let home = std::env::home_dir();
    let apple = (family == OsFamily::MacOs).then(|| apple_facts(home.as_deref()));
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute());

    let (runtimes, graphics_printed) = runtimes_and_graphics(
        &search,
        family,
        apple.as_ref(),
        graphics::job(family, system_root.as_deref()),
    );

    Installed {
        shells: shells(&search, family, std::env::var_os("SHELL").as_deref()),
        package_managers: named_programs(&search, &PACKAGE_MANAGERS),
        runtimes,
        browsers: browsers(
            family,
            &search,
            &BrowserPlaces::on_this_computer(home.as_deref()),
        ),
        graphics: graphics::read(family, graphics_printed.as_deref()),
    }
}

/// The names of the listed programs that are on the `PATH`.
fn named_programs(search: &SearchPath, listed: &[(&'static str, &str)]) -> Vec<&'static str> {
    listed
        .iter()
        .filter(|(_, program)| search.has(program))
        .map(|(name, _)| *name)
        .collect()
}

/// The shells that are on the `PATH`, and the user's login shell.
///
/// `shell_variable` is the value of `SHELL`, which macOS and Linux set to the
/// login shell. Windows has nothing like it.
fn shells(search: &SearchPath, family: OsFamily, shell_variable: Option<&OsStr>) -> Shells {
    let known: &[(&'static str, &str)] = if family == OsFamily::Windows {
        &WINDOWS_SHELLS
    } else {
        &UNIX_SHELLS
    };

    let login = if family == OsFamily::Windows {
        None
    } else {
        shell_variable
            .map(Path::new)
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .and_then(|program| {
                // The name it's listed under, if it's one of the known ones.
                match known.iter().find(|(_, listed)| *listed == program) {
                    Some((name, _)) => Some((*name).to_owned()),
                    None => plain_if_any(program),
                }
            })
    };

    Shells {
        login,
        found: named_programs(search, known),
    }
}

/// The browsers that are installed, found by looking. None is started.
fn browsers(family: OsFamily, search: &SearchPath, places: &BrowserPlaces) -> Vec<&'static str> {
    let is_there = |browser: &Browser| match family {
        OsFamily::MacOs => places
            .applications
            .iter()
            .any(|folder| folder.join(browser.mac_app).is_dir()),
        OsFamily::Windows => places.program_folders.iter().any(|folder| {
            // Joined part by part, so the test works on any OS.
            browser
                .windows_program
                .split('\\')
                .fold(folder.clone(), |path, part| path.join(part))
                .is_file()
        }),
        OsFamily::Linux | OsFamily::Other => {
            browser
                .linux_commands
                .iter()
                .any(|command| search.has(command))
                || places
                    .flatpak_folders
                    .iter()
                    .any(|folder| folder.join(browser.flatpak).exists())
        }
    };

    BROWSERS
        .iter()
        .filter(|browser| is_there(browser))
        .map(|browser| browser.name)
        .collect()
}

/// One runtime being looked for.
struct Lookup {
    name: &'static str,
    args: &'static [&'static str],
    /// The programs found for it that haven't been tried yet.
    untried: VecDeque<PathBuf>,
    /// The answer, once there is one.
    state: Option<RuntimeState>,
    /// Whether one of its programs was Apple's stand-in for the developer
    /// tools, which aren't installed.
    needs_developer_tools: bool,
}

impl Lookup {
    fn new(wanted: &Wanted, search: &SearchPath, family: OsFamily) -> Self {
        let mut programs: Vec<&str> = wanted.programs.to_vec();
        if family == OsFamily::Windows && wanted.name == "Python" {
            programs.push(WINDOWS_PYTHON_LAUNCHER);
        }
        Self {
            name: wanted.name,
            args: wanted.args,
            untried: programs
                .into_iter()
                .filter_map(|program| search.find(program))
                .collect(),
            state: None,
            needs_developer_tools: false,
        }
    }

    /// The next program to run for this runtime, if there's one worth
    /// running. When there's none left, the answer is settled.
    fn next_to_run(&mut self, apple: Option<&AppleFacts>) -> Option<PathBuf> {
        if self.state.is_some() {
            return None;
        }
        while let Some(program) = self.untried.pop_front() {
            match plan_for(&program, apple) {
                Plan::Run => return Some(program),
                Plan::NeedsDeveloperTools => self.needs_developer_tools = true,
                Plan::NothingBehindIt => {}
            }
        }
        self.state = Some(if self.needs_developer_tools {
            RuntimeState::NeedsDeveloperTools
        } else {
            RuntimeState::NotInstalled
        });
        None
    }

    /// Takes what a program did. A program that failed leaves the question
    /// open, so the next one is tried.
    fn take(&mut self, result: Result<Ran, RunError>) {
        match result {
            Ok(ran) if ran.succeeded => {
                self.state = Some(RuntimeState::Found {
                    version: version_in(&ran.stdout).or_else(|| version_in(&ran.stderr)),
                });
            }
            // A stand-in with nothing behind it says so and reports failure.
            // That means "not installed", not that something went wrong.
            Ok(_) | Err(RunError::NotStarted) => {}
            Err(RunError::TimedOut) => self.state = Some(RuntimeState::NoAnswer),
        }
    }
}

/// Looks for every runtime, and asks the graphics question alongside the
/// first round of programs.
///
/// It goes in rounds because a runtime can have more than one program to
/// try, and the second is only run when the first had no answer. There are
/// rarely two rounds, and never more than three.
fn runtimes_and_graphics(
    search: &SearchPath,
    family: OsFamily,
    apple: Option<&AppleFacts>,
    graphics_job: Option<Job>,
) -> (Vec<Runtime>, Option<String>) {
    let mut lookups: Vec<Lookup> = RUNTIMES
        .iter()
        .map(|wanted| Lookup::new(wanted, search, family))
        .collect();
    let mut graphics_job = graphics_job;
    let mut graphics_printed = None;

    loop {
        // Which lookup each job belongs to. `None` is the graphics question.
        let mut owners: Vec<Option<usize>> = Vec::new();
        let mut jobs: Vec<Job> = Vec::new();
        for (index, lookup) in lookups.iter_mut().enumerate() {
            if let Some(program) = lookup.next_to_run(apple) {
                owners.push(Some(index));
                jobs.push(Job {
                    program,
                    args: lookup.args,
                    limit: VERSION_LIMIT,
                });
            }
        }
        if let Some(job) = graphics_job.take() {
            owners.push(None);
            jobs.push(job);
        }
        if jobs.is_empty() {
            break;
        }

        for (owner, result) in owners.into_iter().zip(run::run_all(&jobs, AT_ONCE)) {
            match owner.and_then(|index| lookups.get_mut(index)) {
                Some(lookup) => lookup.take(result),
                None => graphics_printed = result.ok().map(|ran| ran.stdout),
            }
        }
    }

    let runtimes = lookups
        .into_iter()
        .map(|lookup| Runtime {
            name: lookup.name,
            state: lookup.state.unwrap_or(RuntimeState::NotInstalled),
        })
        .collect();
    (runtimes, graphics_printed)
}

/// Decides whether a program that was found is worth running.
fn plan_for(program: &Path, apple: Option<&AppleFacts>) -> Plan {
    let Some(apple) = apple else {
        return Plan::Run;
    };
    // Apple's stand-ins all live in `/usr/bin`. A `git` from anywhere else
    // is a real one.
    if program.parent() != Some(Path::new("/usr/bin")) {
        return Plan::Run;
    }
    match program.file_name().and_then(OsStr::to_str) {
        Some("git" | "python3") if !apple.developer_tools => Plan::NeedsDeveloperTools,
        Some("java") if !apple.java => Plan::NothingBehindIt,
        _ => Plan::Run,
    }
}

/// Finds out what's behind Apple's stand-ins, by looking.
fn apple_facts(home: Option<&Path>) -> AppleFacts {
    let mut developer_folders = vec![
        PathBuf::from("/Library/Developer/CommandLineTools"),
        PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
    ];
    // Wherever `xcode-select` was last pointed.
    developer_folders.extend(std::fs::read_link("/var/db/xcode_select_link").ok());

    let mut java_folders = vec![PathBuf::from("/Library/Java/JavaVirtualMachines")];
    java_folders.extend(home.map(|home| home.join("Library/Java/JavaVirtualMachines")));
    let java_home_is_set = std::env::var_os("JAVA_HOME").is_some_and(|value| !value.is_empty());

    AppleFacts {
        developer_tools: has_developer_tools(&developer_folders),
        java: java_home_is_set || has_a_java(&java_folders),
    }
}

/// Whether any of these folders holds Apple's developer tools.
fn has_developer_tools(folders: &[PathBuf]) -> bool {
    folders
        .iter()
        .any(|folder| folder.join("usr/bin/git").is_file())
}

/// Whether any of these folders holds an installed Java.
fn has_a_java(folders: &[PathBuf]) -> bool {
    folders.iter().any(|folder| {
        std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            // Hidden files, such as `.DS_Store`, aren't a Java.
            .any(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    })
}

/// Picks the version out of what a program printed.
///
/// Programs say it in different ways, such as `git version 2.50.1`,
/// `v24.13.0` and `openjdk version "25.0.4.1" 2026-08-18`. The version is the
/// first word that looks like one. Only that word is kept, never the rest of
/// what the program said.
fn version_in(printed: &str) -> Option<String> {
    printed
        .lines()
        .take(LINES_SEARCHED)
        .flat_map(str::split_whitespace)
        .find_map(as_version)
}

/// The word as a version, if it looks like one.
fn as_version(word: &str) -> Option<String> {
    let unquoted = word.trim_matches('"');
    let was_quoted = unquoted.len() < word.len();
    let word = unquoted.trim_end_matches([',', ';', ')']);
    // `v24.13.0` is version 24.13.0.
    let word = match word.strip_prefix('v') {
        Some(rest) if rest.starts_with(|first: char| first.is_ascii_digit()) => rest,
        _ => word,
    };

    let looks_right = word.starts_with(|first: char| first.is_ascii_digit())
        && word.len() <= MOST_VERSION
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        // A bare number is only a version when the program quoted it, as
        // Java does. Otherwise it could be a year or a build number.
        && (was_quoted || word.contains('.'));
    looks_right.then(|| word.to_owned())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        AppleFacts, BrowserPlaces, Lookup, Plan, RUNTIMES, RuntimeState, Shells, browsers,
        has_a_java, has_developer_tools, named_programs, plan_for, shells, version_in,
    };
    use crate::profile::path_search::SearchPath;
    use crate::profile::run::{Ran, RunError};
    use crate::profile::{OsFamily, test_folder};

    /// Puts a program called `name` in `folder`.
    fn add_program(folder: &Path, name: &str) {
        fs::create_dir_all(folder).expect("the folder");
        let file = folder.join(name);
        fs::write(&file, "").expect("the program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).expect("the mode");
        }
    }

    /// A search path over one folder, in the style of macOS and Linux.
    fn search_in(folder: &Path) -> SearchPath {
        SearchPath::new(Some(folder.as_os_str()), None)
    }

    fn nothing_on_the_path() -> SearchPath {
        SearchPath::new(None, None)
    }

    #[test]
    fn versions_are_read_from_what_real_programs_print() {
        let printed = [
            ("git version 2.50.1 (Apple Git-155)\n", "2.50.1"),
            ("git version 2.47.1.windows.2\n", "2.47.1.windows.2"),
            ("Python 3.13.1\n", "3.13.1"),
            ("v24.13.0\n", "24.13.0"),
            ("Docker version 28.3.2, build 578ccf6\n", "28.3.2"),
            (
                "openjdk version \"25.0.4.1\" 2026-08-18 LTS\nOpenJDK Runtime Environment Temurin-25.0.4.1+1\n",
                "25.0.4.1",
            ),
            ("java version \"1.8.0_391\"\n", "1.8.0_391"),
            // The first release of a Java version has no dots at all.
            ("openjdk version \"21\" 2023-09-19\n", "21"),
            (
                "Picked up JAVA_TOOL_OPTIONS: -Dfile.encoding=UTF8\nopenjdk version \"17.0.9\" 2023-10-17\n",
                "17.0.9",
            ),
        ];
        for (text, expected) in printed {
            assert_eq!(version_in(text).as_deref(), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn text_with_no_version_in_it_gives_none() {
        let printed = [
            "",
            "Python was not found; run without arguments to install from the Microsoft Store.\n",
            "The operation couldn't be completed. Unable to locate a Java Runtime.\n",
            "usage: tool [options]\n",
            // A year and a build number aren't versions.
            "Copyright 2026 build 578\n",
            // Too long to be one.
            "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17\n",
            // A path isn't one, and neither is anything with odd characters.
            "/Users/sam/.pyenv/versions/3.9\n",
            "3.9;rm\n",
        ];
        for text in printed {
            assert_eq!(version_in(text), None, "{text:?}");
        }
    }

    #[test]
    fn only_the_first_lines_of_the_output_are_searched() {
        let late = format!("{}version 1.2.3\n", "nothing here\n".repeat(50));

        assert_eq!(version_in(&late), None);
    }

    #[test]
    fn shells_and_package_managers_are_the_ones_on_the_path() {
        let folder = test_folder("installed-shells");
        for program in ["zsh", "bash", "sh", "brew", "pwsh"] {
            add_program(folder.path(), program);
        }
        let search = search_in(folder.path());

        assert_eq!(
            shells(&search, OsFamily::MacOs, Some(OsStr::new("/bin/zsh"))),
            Shells {
                login: Some("zsh".to_owned()),
                found: vec!["sh", "bash", "zsh", "PowerShell"],
            }
        );
        assert_eq!(
            named_programs(&search, &super::PACKAGE_MANAGERS),
            ["Homebrew"]
        );
    }

    #[test]
    fn a_login_shell_wrybill_does_not_know_is_still_named() {
        let search = nothing_on_the_path();

        let found = shells(&search, OsFamily::Linux, Some(OsStr::new("/usr/bin/nu")));
        assert_eq!(found.login.as_deref(), Some("nu"));
        assert!(found.found.is_empty());

        let powershell = shells(&search, OsFamily::Linux, Some(OsStr::new("/usr/bin/pwsh")));
        assert_eq!(powershell.login.as_deref(), Some("PowerShell"));

        assert_eq!(shells(&search, OsFamily::Linux, None).login, None);
    }

    #[test]
    fn windows_has_its_own_shells_and_no_login_shell() {
        let folder = test_folder("installed-windows-shells");
        for program in ["cmd.exe", "powershell.exe", "bash.exe"] {
            add_program(folder.path(), program);
        }
        let search = SearchPath::new(Some(folder.path().as_os_str()), Some(".COM;.EXE"));

        assert_eq!(
            shells(&search, OsFamily::Windows, Some(OsStr::new("/bin/zsh"))),
            Shells {
                login: None,
                found: vec!["cmd", "Windows PowerShell", "bash"],
            }
        );
    }

    #[test]
    fn mac_browsers_are_found_in_the_applications_folders() {
        let folder = test_folder("installed-mac-browsers");
        let (system, user) = (folder.path().join("system"), folder.path().join("user"));
        fs::create_dir_all(system.join("Firefox.app")).expect("an app");
        fs::create_dir_all(user.join("Google Chrome.app")).expect("an app");
        // A file with an app's name isn't an app.
        fs::write(system.join("Chromium.app"), "").expect("a file");
        let places = BrowserPlaces {
            applications: vec![system, user],
            ..BrowserPlaces::default()
        };

        assert_eq!(
            browsers(OsFamily::MacOs, &nothing_on_the_path(), &places),
            ["Chrome", "Firefox"]
        );
    }

    #[test]
    fn windows_browsers_are_found_where_programs_install() {
        let folder = test_folder("installed-windows-browsers");
        let (all_users, this_user) = (folder.path().join("pf"), folder.path().join("local"));
        let edge = all_users.join("Microsoft").join("Edge").join("Application");
        let brave = this_user
            .join("BraveSoftware")
            .join("Brave-Browser")
            .join("Application");
        fs::create_dir_all(&edge).expect("a folder");
        fs::create_dir_all(&brave).expect("a folder");
        fs::write(edge.join("msedge.exe"), "").expect("a program");
        fs::write(brave.join("brave.exe"), "").expect("a program");
        let places = BrowserPlaces {
            program_folders: vec![all_users, this_user],
            ..BrowserPlaces::default()
        };

        assert_eq!(
            browsers(OsFamily::Windows, &nothing_on_the_path(), &places),
            ["Edge", "Brave"]
        );
    }

    #[test]
    fn linux_browsers_are_found_on_the_path_or_as_flatpaks() {
        let folder = test_folder("installed-linux-browsers");
        let (bin, flatpak) = (folder.path().join("bin"), folder.path().join("flatpak"));
        add_program(&bin, "firefox-esr");
        add_program(&bin, "chromium-browser");
        fs::create_dir_all(&flatpak).expect("a folder");
        fs::write(flatpak.join("com.brave.Browser"), "").expect("a launcher");
        let places = BrowserPlaces {
            flatpak_folders: vec![flatpak],
            ..BrowserPlaces::default()
        };

        assert_eq!(
            browsers(OsFamily::Linux, &search_in(&bin), &places),
            ["Brave", "Chromium", "Firefox"]
        );
    }

    #[test]
    fn with_no_browser_anywhere_none_is_listed() {
        for family in [OsFamily::MacOs, OsFamily::Windows, OsFamily::Linux] {
            assert!(
                browsers(family, &nothing_on_the_path(), &BrowserPlaces::default()).is_empty(),
                "{family:?}"
            );
        }
    }

    #[test]
    fn off_a_mac_every_program_that_is_found_is_run() {
        for program in ["/usr/bin/git", "/usr/bin/java", "/usr/bin/python3"] {
            assert_eq!(plan_for(Path::new(program), None), Plan::Run, "{program}");
        }
    }

    #[test]
    fn apples_stand_ins_are_not_run_with_nothing_behind_them() {
        let bare = AppleFacts {
            developer_tools: false,
            java: false,
        };

        assert_eq!(
            plan_for(Path::new("/usr/bin/git"), Some(&bare)),
            Plan::NeedsDeveloperTools
        );
        assert_eq!(
            plan_for(Path::new("/usr/bin/python3"), Some(&bare)),
            Plan::NeedsDeveloperTools
        );
        assert_eq!(
            plan_for(Path::new("/usr/bin/java"), Some(&bare)),
            Plan::NothingBehindIt
        );
        // Programs from anywhere else are real ones, and so is anything
        // else in `/usr/bin`.
        for real in [
            "/opt/homebrew/bin/git",
            "/usr/local/bin/python3",
            "/opt/homebrew/opt/openjdk/bin/java",
            "/usr/bin/zsh",
        ] {
            assert_eq!(plan_for(Path::new(real), Some(&bare)), Plan::Run, "{real}");
        }
    }

    #[test]
    fn apples_stand_ins_are_run_once_something_is_behind_them() {
        let set_up = AppleFacts {
            developer_tools: true,
            java: true,
        };

        for program in ["/usr/bin/git", "/usr/bin/python3", "/usr/bin/java"] {
            assert_eq!(
                plan_for(Path::new(program), Some(&set_up)),
                Plan::Run,
                "{program}"
            );
        }
    }

    #[test]
    fn the_developer_tools_are_found_by_their_git() {
        let folder = test_folder("installed-developer-tools");
        let (tools, empty) = (folder.path().join("tools"), folder.path().join("empty"));
        fs::create_dir_all(tools.join("usr/bin")).expect("a folder");
        fs::write(tools.join("usr/bin/git"), "").expect("git");
        fs::create_dir_all(&empty).expect("a folder");

        assert!(has_developer_tools(&[empty.clone(), tools]));
        assert!(!has_developer_tools(&[
            empty,
            folder.path().join("missing")
        ]));
        assert!(!has_developer_tools(&[]));
    }

    #[test]
    fn a_java_is_found_by_its_folder() {
        let folder = test_folder("installed-java");
        let (with_java, without) = (folder.path().join("with"), folder.path().join("without"));
        fs::create_dir_all(with_java.join("temurin-25.jdk")).expect("a folder");
        fs::create_dir_all(&without).expect("a folder");
        fs::write(without.join(".DS_Store"), "").expect("a hidden file");

        assert!(has_a_java(&[without.clone(), with_java]));
        assert!(!has_a_java(&[without, folder.path().join("missing")]));
    }

    /// A lookup for Python with these programs found for it.
    fn python_lookup(found: &[&str]) -> Lookup {
        let python = RUNTIMES
            .iter()
            .find(|wanted| wanted.name == "Python")
            .expect("Python is one of the runtimes");
        let mut lookup = Lookup::new(python, &nothing_on_the_path(), OsFamily::Linux);
        lookup.untried = found.iter().map(PathBuf::from).collect();
        lookup
    }

    fn ran(succeeded: bool, stdout: &str) -> Result<Ran, RunError> {
        Ok(Ran {
            succeeded,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    #[test]
    fn a_runtime_with_no_program_on_the_path_is_not_installed() {
        let mut lookup = python_lookup(&[]);

        assert_eq!(lookup.next_to_run(None), None);
        assert_eq!(lookup.state, Some(RuntimeState::NotInstalled));
    }

    #[test]
    fn a_stand_in_that_reports_failure_means_the_next_program_is_tried() {
        // On Windows, `python3` can be a stand-in that only offers to
        // install Python from the Microsoft Store.
        let mut lookup = python_lookup(&["/apps/python3", "/real/python"]);

        assert_eq!(
            lookup.next_to_run(None),
            Some(PathBuf::from("/apps/python3"))
        );
        lookup.take(ran(false, "Python was not found; run without arguments\n"));
        assert_eq!(lookup.state, None);

        assert_eq!(
            lookup.next_to_run(None),
            Some(PathBuf::from("/real/python"))
        );
        lookup.take(ran(true, "Python 3.13.1\n"));
        assert_eq!(
            lookup.state,
            Some(RuntimeState::Found {
                version: Some("3.13.1".to_owned())
            })
        );
        // Once there's an answer, nothing more is run.
        assert_eq!(lookup.next_to_run(None), None);
    }

    #[test]
    fn when_every_program_reports_failure_the_runtime_is_not_installed() {
        let mut lookup = python_lookup(&["/apps/python3"]);

        lookup.next_to_run(None);
        lookup.take(ran(false, ""));

        assert_eq!(lookup.next_to_run(None), None);
        assert_eq!(lookup.state, Some(RuntimeState::NotInstalled));
    }

    #[test]
    fn a_version_printed_as_an_error_is_read_too() {
        let mut lookup = python_lookup(&["/usr/bin/java"]);

        lookup.next_to_run(None);
        lookup.take(Ok(Ran {
            succeeded: true,
            stdout: String::new(),
            stderr: "openjdk version \"25.0.4.1\" 2026-08-18 LTS\n".to_owned(),
        }));

        assert_eq!(
            lookup.state,
            Some(RuntimeState::Found {
                version: Some("25.0.4.1".to_owned())
            })
        );
    }

    #[test]
    fn a_program_that_answers_without_a_version_is_still_installed() {
        let mut lookup = python_lookup(&["/usr/bin/python3"]);

        lookup.next_to_run(None);
        lookup.take(ran(true, "hello\n"));

        assert_eq!(lookup.state, Some(RuntimeState::Found { version: None }));
    }

    #[test]
    fn a_program_that_does_not_answer_in_time_is_reported_as_such() {
        let mut lookup = python_lookup(&["/usr/bin/python3", "/usr/bin/python"]);

        lookup.next_to_run(None);
        lookup.take(Err(RunError::TimedOut));

        assert_eq!(lookup.state, Some(RuntimeState::NoAnswer));
        assert_eq!(lookup.next_to_run(None), None);
    }

    #[test]
    fn a_mac_without_the_developer_tools_says_so_and_runs_nothing() {
        let bare = AppleFacts {
            developer_tools: false,
            java: false,
        };
        let mut lookup = python_lookup(&["/usr/bin/python3"]);

        assert_eq!(lookup.next_to_run(Some(&bare)), None);
        assert_eq!(lookup.state, Some(RuntimeState::NeedsDeveloperTools));
    }

    #[test]
    fn a_real_program_is_still_used_on_a_mac_without_the_developer_tools() {
        let bare = AppleFacts {
            developer_tools: false,
            java: false,
        };
        let mut lookup = python_lookup(&["/usr/bin/python3", "/opt/homebrew/bin/python"]);

        assert_eq!(
            lookup.next_to_run(Some(&bare)),
            Some(PathBuf::from("/opt/homebrew/bin/python"))
        );
    }

    #[test]
    fn the_windows_launcher_is_only_looked_for_on_windows() {
        let folder = test_folder("installed-py-launcher");
        add_program(folder.path(), "py.exe");
        add_program(folder.path(), "py");
        let python = RUNTIMES
            .iter()
            .find(|wanted| wanted.name == "Python")
            .expect("Python is one of the runtimes");
        let windows_search = SearchPath::new(Some(folder.path().as_os_str()), Some(".EXE"));
        let unix_search = search_in(folder.path());

        let on_windows = Lookup::new(python, &windows_search, OsFamily::Windows);
        assert_eq!(on_windows.untried, [folder.path().join("py.exe")]);

        let on_linux = Lookup::new(python, &unix_search, OsFamily::Linux);
        assert!(on_linux.untried.is_empty());
    }
}
