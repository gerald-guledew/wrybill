//! Finds programs by looking along the `PATH`. Nothing is run.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The endings a program's file has on Windows, when `PATHEXT` doesn't say.
const USUAL_ENDINGS: &str = ".COM;.EXE;.BAT;.CMD";

/// The folders to look for programs in, first choice first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SearchPath {
    folders: Vec<PathBuf>,
    /// The endings that make a file a program, in lowercase, such as `.exe`.
    /// Empty on macOS and Linux, where a program's name is the whole file
    /// name.
    endings: Vec<String>,
}

impl SearchPath {
    /// The search path of this process: `PATH`, and on Windows `PATHEXT`.
    pub(super) fn from_env() -> Self {
        let path = std::env::var_os("PATH");
        if cfg!(windows) {
            let endings = std::env::var("PATHEXT").unwrap_or_default();
            Self::new(path.as_deref(), Some(&endings))
        } else {
            Self::new(path.as_deref(), None)
        }
    }

    /// A search path from the value of `PATH`.
    ///
    /// `endings` is the value of `PATHEXT`, for a Windows-style search. With
    /// `None`, a program's name is the whole file name.
    ///
    /// Folders that aren't full paths are left out, so the current folder is
    /// never searched.
    pub(super) fn new(path: Option<&OsStr>, endings: Option<&str>) -> Self {
        let folders = path
            .map(|path| {
                std::env::split_paths(path)
                    .filter(|folder| folder.is_absolute())
                    .collect()
            })
            .unwrap_or_default();

        let endings = match endings {
            Some(listed) => {
                let listed = parse_endings(listed);
                if listed.is_empty() {
                    parse_endings(USUAL_ENDINGS)
                } else {
                    listed
                }
            }
            None => Vec::new(),
        };

        Self { folders, endings }
    }

    /// The first program called `name` along the path.
    pub(super) fn find(&self, name: &str) -> Option<PathBuf> {
        self.folders
            .iter()
            .find_map(|folder| self.find_in(folder, name))
    }

    /// Whether there's a program called `name` along the path.
    pub(super) fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    fn find_in(&self, folder: &Path, name: &str) -> Option<PathBuf> {
        if self.endings.is_empty() {
            let candidate = folder.join(name);
            return is_program(&candidate).then_some(candidate);
        }
        self.endings
            .iter()
            .map(|ending| folder.join(format!("{name}{ending}")))
            .find(|candidate| is_program(candidate))
    }
}

fn parse_endings(listed: &str) -> Vec<String> {
    listed
        .split(';')
        .map(str::trim)
        .filter(|ending| ending.len() > 1 && ending.starts_with('.'))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Whether a file is something that can be run.
#[cfg(unix)]
fn is_program(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(file)
        .is_ok_and(|about| about.is_file() && about.permissions().mode() & 0o111 != 0)
}

/// Whether a file is something that can be run.
#[cfg(not(unix))]
fn is_program(file: &Path) -> bool {
    // Windows has "app execution aliases", which aren't plain files and can't
    // be opened like one. So anything that's there and isn't a folder counts.
    std::fs::symlink_metadata(file).is_ok_and(|about| !about.is_dir())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::SearchPath;
    use crate::profile::test_folder;

    /// Puts a program called `file_name` in `folder`.
    fn add_program(folder: &Path, file_name: &str) -> PathBuf {
        fs::create_dir_all(folder).expect("the folder");
        let file = folder.join(file_name);
        fs::write(&file, "").expect("the program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).expect("the mode");
        }
        file
    }

    fn path_of(folders: &[&Path]) -> OsString {
        std::env::join_paths(folders).expect("a PATH")
    }

    #[test]
    fn the_first_folder_with_the_program_wins() {
        let root = test_folder("path-first-wins");
        let (first, second) = (root.path().join("first"), root.path().join("second"));
        add_program(&second, "tool");
        let in_first = add_program(&first, "other");
        let in_second = second.join("tool");

        let search = SearchPath::new(Some(&path_of(&[&first, &second])), None);

        assert_eq!(search.find("tool"), Some(in_second));
        assert_eq!(search.find("other"), Some(in_first));
        assert_eq!(search.find("missing"), None);
        assert!(search.has("tool"));
        assert!(!search.has("missing"));
    }

    #[test]
    fn a_folder_with_the_right_name_is_not_a_program() {
        let root = test_folder("path-folder");
        let bin = root.path().join("bin");
        fs::create_dir_all(bin.join("tool")).expect("a folder");

        let search = SearchPath::new(Some(&path_of(&[&bin])), None);

        assert_eq!(search.find("tool"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_run_is_not_a_program() {
        let root = test_folder("path-not-runnable");
        fs::create_dir_all(root.path()).expect("the folder");
        fs::write(root.path().join("tool"), "").expect("a file");

        let search = SearchPath::new(Some(&path_of(&[root.path()])), None);

        assert_eq!(search.find("tool"), None);
    }

    #[test]
    fn a_windows_style_search_tries_each_ending_in_turn() {
        let root = test_folder("path-endings");
        let as_cmd = add_program(root.path(), "tool.cmd");
        add_program(root.path(), "plain");

        let search = SearchPath::new(Some(&path_of(&[root.path()])), Some(".COM;.EXE; .CMD ;;x"));

        assert_eq!(search.find("tool"), Some(as_cmd));
        // With no ending, a file isn't a program on Windows.
        assert_eq!(search.find("plain"), None);
    }

    #[test]
    fn the_usual_endings_stand_in_when_pathext_is_empty() {
        let root = test_folder("path-usual-endings");
        let as_exe = add_program(root.path(), "tool.exe");

        let search = SearchPath::new(Some(&path_of(&[root.path()])), Some(""));

        assert_eq!(search.find("tool"), Some(as_exe));
    }

    #[test]
    fn folders_that_are_not_full_paths_are_never_searched() {
        let relative = OsString::from("bin");
        let search = SearchPath::new(Some(&relative), None);
        let empty = SearchPath::new(None, None);

        assert_eq!(search.find("anything"), None);
        assert_eq!(empty.find("anything"), None);
    }
}
