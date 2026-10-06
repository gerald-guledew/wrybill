//! Where Wrybill keeps its data on this machine (spec 13.1 and 15.2).
//!
//! Everything here is worked out from values handed in, so tests never touch
//! the real environment. Only [`Paths::from_env`] reads it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The data folder's name inside the user's home folder.
const DATA_FOLDER_NAME: &str = ".wrybill";

/// The config file's name inside the data folder.
const CONFIG_FILE_NAME: &str = "config.toml";

/// The name of the folder for Wrybill's own logs, inside the data folder.
const LOGS_FOLDER_NAME: &str = "logs";

/// The folders and files Wrybill uses on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    data_folder: PathBuf,
    user_home: Option<PathBuf>,
}

/// Why the paths couldn't be worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathsError {
    /// `WRYBILL_HOME` is set, but not to a full path.
    #[error(
        "WRYBILL_HOME must be the full path of a folder. It can't start from the current folder or with ~."
    )]
    HomeNotAbsolute,
    /// There's no `WRYBILL_HOME`, and the user's home folder is unknown.
    #[error(
        "Wrybill can't find your home folder. Set WRYBILL_HOME to the full path of the folder Wrybill should keep its data in."
    )]
    NoUserHome,
}

impl Paths {
    /// Works the paths out from the real environment: `WRYBILL_HOME` if it's
    /// set, and otherwise `.wrybill` in the user's home folder.
    pub fn from_env() -> Result<Self, PathsError> {
        Self::resolve(std::env::var_os("WRYBILL_HOME"), std::env::home_dir())
    }

    /// Works the paths out from the two values given. An empty `WRYBILL_HOME`
    /// counts as not set.
    pub fn resolve(
        wrybill_home: Option<OsString>,
        user_home: Option<PathBuf>,
    ) -> Result<Self, PathsError> {
        // A home folder that isn't a full path is no use for expanding `~`.
        let user_home = user_home.filter(|home| home.is_absolute());

        let data_folder = match wrybill_home.filter(|value| !value.is_empty()) {
            Some(value) => {
                let folder = PathBuf::from(value);
                if !folder.is_absolute() {
                    return Err(PathsError::HomeNotAbsolute);
                }
                folder
            }
            None => user_home
                .as_ref()
                .ok_or(PathsError::NoUserHome)?
                .join(DATA_FOLDER_NAME),
        };

        Ok(Self {
            data_folder,
            user_home,
        })
    }

    /// The folder that holds all of Wrybill's data.
    pub fn data_folder(&self) -> &Path {
        &self.data_folder
    }

    /// Whether the data folder is where it is by default: `.wrybill` in the
    /// user's home folder.
    pub fn data_folder_is_default(&self) -> bool {
        self.user_home
            .as_ref()
            .is_some_and(|home| self.data_folder == home.join(DATA_FOLDER_NAME))
    }

    /// Where the config file is, whether or not it exists.
    pub fn config_file(&self) -> PathBuf {
        self.data_folder.join(CONFIG_FILE_NAME)
    }

    /// Where Wrybill's own logs go, whether or not the folder exists yet.
    pub fn logs_folder(&self) -> PathBuf {
        self.data_folder.join(LOGS_FOLDER_NAME)
    }

    /// The user's home folder, when it's known. Used to expand `~`.
    pub fn user_home(&self) -> Option<&Path> {
        self.user_home.as_deref()
    }
}

/// Replaces a leading `~` with the user's home folder.
///
/// Only a `~` on its own, or one followed by a path separator, is expanded.
/// Any other text comes back as the path it already is. It's an error only
/// when there's a `~` to expand and the home folder isn't known.
pub fn expand_tilde(text: &str, user_home: Option<&Path>) -> Result<PathBuf, PathsError> {
    let Some(rest) = text.strip_prefix('~') else {
        return Ok(PathBuf::from(text));
    };
    if !rest.is_empty() && !rest.starts_with(std::path::is_separator) {
        // Something like `~name`, which Wrybill doesn't expand.
        return Ok(PathBuf::from(text));
    }

    let mut path = user_home.ok_or(PathsError::NoUserHome)?.to_path_buf();
    // Pushed part by part, so the result uses this OS's own separator.
    for part in rest.split(std::path::is_separator) {
        if !part.is_empty() {
            path.push(part);
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{Paths, PathsError, expand_tilde};

    /// A full path that is valid on the OS the tests run on.
    fn full(path: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!(r"C:\{}", path.replace('/', r"\")))
        } else {
            PathBuf::from(format!("/{path}"))
        }
    }

    #[test]
    fn the_data_folder_is_dot_wrybill_in_the_home_folder() {
        let paths = Paths::resolve(None, Some(full("home/sam"))).expect("paths");

        assert_eq!(paths.data_folder(), full("home/sam/.wrybill"));
        assert_eq!(paths.config_file(), full("home/sam/.wrybill/config.toml"));
        assert_eq!(paths.logs_folder(), full("home/sam/.wrybill/logs"));
        assert_eq!(paths.user_home(), Some(full("home/sam").as_path()));
    }

    #[test]
    fn the_data_folder_knows_whether_it_was_moved() {
        let home = full("home/sam");
        let by_default = Paths::resolve(None, Some(home.clone())).expect("paths");
        let moved_inside_home = Paths::resolve(
            Some(OsString::from(full("home/sam/projects/wrybill-data"))),
            Some(home.clone()),
        )
        .expect("paths");
        let moved_elsewhere =
            Paths::resolve(Some(OsString::from(full("data/wrybill"))), Some(home)).expect("paths");
        let with_no_home =
            Paths::resolve(Some(OsString::from(full("data/wrybill"))), None).expect("paths");

        assert!(by_default.data_folder_is_default());
        assert!(!moved_inside_home.data_folder_is_default());
        assert!(!moved_elsewhere.data_folder_is_default());
        assert!(!with_no_home.data_folder_is_default());
    }

    #[test]
    fn wrybill_home_moves_the_whole_folder() {
        let moved = OsString::from(full("data/wrybill"));

        let paths = Paths::resolve(Some(moved), Some(full("home/sam"))).expect("paths");

        assert_eq!(paths.data_folder(), full("data/wrybill"));
        assert_eq!(paths.config_file(), full("data/wrybill/config.toml"));
    }

    #[test]
    fn wrybill_home_works_without_a_home_folder() {
        let moved = OsString::from(full("data/wrybill"));

        let paths = Paths::resolve(Some(moved), None).expect("paths");

        assert_eq!(paths.data_folder(), full("data/wrybill"));
        assert_eq!(paths.user_home(), None);
    }

    #[test]
    fn an_empty_wrybill_home_counts_as_not_set() {
        let paths = Paths::resolve(Some(OsString::new()), Some(full("home/sam"))).expect("paths");

        assert_eq!(paths.data_folder(), full("home/sam/.wrybill"));
    }

    #[test]
    fn wrybill_home_must_be_a_full_path() {
        for value in ["wrybill-data", "./wrybill-data", "~/wrybill-data"] {
            let result = Paths::resolve(Some(OsString::from(value)), Some(full("home/sam")));

            assert_eq!(result, Err(PathsError::HomeNotAbsolute), "{value}");
        }
    }

    #[test]
    fn with_no_home_folder_and_no_wrybill_home_there_is_nowhere_to_go() {
        assert_eq!(Paths::resolve(None, None), Err(PathsError::NoUserHome));
    }

    #[test]
    fn a_home_folder_that_is_not_a_full_path_is_ignored() {
        let result = Paths::resolve(None, Some(PathBuf::from("sam")));

        assert_eq!(result, Err(PathsError::NoUserHome));
    }

    #[test]
    fn a_tilde_becomes_the_home_folder() {
        let home = full("home/sam");

        assert_eq!(expand_tilde("~", Some(&home)), Ok(home.clone()));
        assert_eq!(
            expand_tilde("~/Wrybill/workspaces", Some(&home)),
            Ok(full("home/sam/Wrybill/workspaces"))
        );
    }

    #[test]
    fn text_without_a_leading_tilde_is_left_alone() {
        let home = full("home/sam");

        for text in ["Projects", "a/~/b", "~sam/Projects"] {
            assert_eq!(
                expand_tilde(text, Some(&home)),
                Ok(PathBuf::from(text)),
                "{text}"
            );
        }
        assert_eq!(
            expand_tilde(&full("srv/work").to_string_lossy(), None),
            Ok(full("srv/work"))
        );
    }

    #[test]
    fn a_tilde_with_no_known_home_folder_is_an_error() {
        assert_eq!(
            expand_tilde("~/Projects", None),
            Err(PathsError::NoUserHome)
        );
    }

    #[test]
    fn an_expanded_path_uses_this_systems_separator() {
        let expanded = expand_tilde("~/a/b", Some(&full("home/sam"))).expect("a path");

        assert_eq!(expanded, Path::new(&full("home/sam")).join("a").join("b"));
    }
}
