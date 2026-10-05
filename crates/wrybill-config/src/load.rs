//! Reads `config.toml` and turns it into a checked [`Config`].

use std::io;
use std::path::Path;

use toml::de::DeTable;

use crate::model::Config;
use crate::problem::Problem;
use crate::reader::Report;
use crate::validate;

/// A config that passed every check.
#[derive(Debug, Clone, PartialEq)]
pub struct Valid {
    /// The settings, with every default filled in.
    pub config: Config,
    /// Things that are allowed but worth a second look.
    pub warnings: Vec<Problem>,
}

/// A config that didn't pass. Nothing in it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    /// Everything that's wrong, in the order of the file.
    pub errors: Vec<Problem>,
    /// Things that are allowed but worth a second look.
    pub warnings: Vec<Problem>,
}

/// Where the settings in use came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The config file.
    File,
    /// The built-in defaults, because there's no config file.
    Defaults,
}

/// The settings Wrybill will run on.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The settings, with every default filled in.
    pub config: Config,
    /// Whether they came from the file or are the built-in defaults.
    pub source: Source,
    /// Things in the file that are allowed but worth a second look.
    pub warnings: Vec<Problem>,
}

/// Why the config file couldn't be used.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file is there, but it couldn't be read.
    #[error("the config file can't be read: {0}")]
    Unreadable(String),
    /// The file was read, and it has problems.
    #[error("the config file has problems")]
    Invalid(Invalid),
}

/// Loads the config file at `path`.
///
/// A missing file isn't an error: Wrybill starts on built-in defaults
/// (spec 13.1). `user_home` is used to expand `~` in folder paths.
pub fn load(path: &Path, user_home: Option<&Path>) -> Result<Loaded, LoadError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Loaded {
                config: Config::defaults(user_home),
                source: Source::Defaults,
                warnings: Vec::new(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Err(LoadError::Unreadable(
                "it isn't plain UTF-8 text".to_owned(),
            ));
        }
        Err(error) => return Err(LoadError::Unreadable(error.to_string())),
    };

    match parse(&text, user_home) {
        Ok(valid) => Ok(Loaded {
            config: valid.config,
            source: Source::File,
            warnings: valid.warnings,
        }),
        Err(invalid) => Err(LoadError::Invalid(invalid)),
    }
}

/// Checks the text of a config file against spec 13.3.
///
/// Every problem is reported at once, each with its line. No problem ever
/// shows the value of an `api_key`, a URL or a path.
pub fn parse(text: &str, user_home: Option<&Path>) -> Result<Valid, Invalid> {
    // Some Windows editors put a byte-order mark at the start of the file.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut report = Report::new(text);

    let table = match DeTable::parse(text) {
        Ok(table) => table,
        Err(error) => {
            let line = error.span().map(|span| report.line_at(span.start));
            report.error(
                line,
                "",
                format!(
                    "Wrybill can't read the file from here on. It isn't valid TOML: {}.",
                    error.message()
                ),
            );
            let (errors, warnings) = report.finish();
            return Err(Invalid { errors, warnings });
        }
    };

    let draft = validate::read(table.get_ref(), user_home, &mut report);
    let has_errors = report.has_errors();
    let (errors, warnings) = report.finish();
    if has_errors {
        Err(Invalid { errors, warnings })
    } else {
        Ok(Valid {
            config: validate::build(draft),
            warnings,
        })
    }
}
