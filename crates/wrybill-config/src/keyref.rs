//! References to API keys.
//!
//! The config file holds these and never the keys themselves (spec 11.6 and
//! 13.3). A key saved with `wrybill keys set <name>` is found by its name.

use std::fmt;

/// The keychain service every Wrybill key is saved under.
pub const KEYCHAIN_SERVICE: &str = "wrybill";

const KEYCHAIN_PREFIX: &str = "keychain:";
const ENV_PREFIX: &str = "env:";

/// The longest a key's name can be. Far more than any provider's name needs.
const NAME_MAX: usize = 64;

/// Where an API key is kept.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyRef {
    /// A key in the OS keychain, saved by `wrybill keys set <name>`.
    /// Written `keychain:wrybill/<name>`.
    Keychain {
        /// The name the key was saved under, such as `anthropic`.
        name: String,
    },
    /// A key in an environment variable, for machines where no keychain can
    /// be reached. Written `env:VARIABLE_NAME`.
    Env {
        /// The variable's name, such as `ANTHROPIC_API_KEY`.
        variable: String,
    },
}

/// Why some text isn't a key reference. It never holds the text itself,
/// because the text may well be a real key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyRefError {
    /// It starts like neither kind of reference, so it may be a key.
    #[error("not a key reference")]
    NotAReference,
    /// It starts with `keychain:`, but the rest isn't `wrybill/<name>`.
    #[error("not a valid keychain reference")]
    BadKeychainReference,
    /// It starts with `env:`, but the rest isn't a variable name.
    #[error("not a valid environment reference")]
    BadEnvReference,
}

impl KeyRef {
    /// The reference to the key saved under `name`, or `None` if the name
    /// isn't one a key can have.
    pub fn keychain(name: &str) -> Option<Self> {
        is_valid_key_name(name).then(|| Self::Keychain {
            name: name.to_owned(),
        })
    }

    /// Reads a reference as the config file writes it.
    pub fn parse(text: &str) -> Result<Self, KeyRefError> {
        if let Some(rest) = text.strip_prefix(KEYCHAIN_PREFIX) {
            rest.strip_prefix(KEYCHAIN_SERVICE)
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(Self::keychain)
                .ok_or(KeyRefError::BadKeychainReference)
        } else if let Some(variable) = text.strip_prefix(ENV_PREFIX) {
            if is_valid_variable_name(variable) {
                Ok(Self::Env {
                    variable: variable.to_owned(),
                })
            } else {
                Err(KeyRefError::BadEnvReference)
            }
        } else {
            Err(KeyRefError::NotAReference)
        }
    }
}

impl fmt::Display for KeyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keychain { name } => write!(f, "{KEYCHAIN_PREFIX}{KEYCHAIN_SERVICE}/{name}"),
            Self::Env { variable } => write!(f, "{ENV_PREFIX}{variable}"),
        }
    }
}

/// Whether a key can be saved under this name: lowercase letters, digits,
/// `-` and `_` (spec 13.1).
pub fn is_valid_key_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_MAX
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

/// Whether this is the name of an environment variable: letters, digits and
/// `_`, not starting with a digit.
pub(crate) fn is_valid_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let starts_well = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
    starts_well && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::{KeyRef, KeyRefError, is_valid_key_name};

    #[test]
    fn a_keychain_reference_round_trips() {
        let reference = KeyRef::parse("keychain:wrybill/anthropic").expect("a valid reference");

        assert_eq!(
            reference,
            KeyRef::Keychain {
                name: "anthropic".to_owned()
            }
        );
        assert_eq!(reference.to_string(), "keychain:wrybill/anthropic");
    }

    #[test]
    fn an_env_reference_round_trips() {
        let reference = KeyRef::parse("env:ANTHROPIC_API_KEY").expect("a valid reference");

        assert_eq!(
            reference,
            KeyRef::Env {
                variable: "ANTHROPIC_API_KEY".to_owned()
            }
        );
        assert_eq!(reference.to_string(), "env:ANTHROPIC_API_KEY");
    }

    #[test]
    fn anything_else_is_not_a_reference() {
        for text in ["", "sk-not-a-real-key", "anthropic", "Keychain:wrybill/x"] {
            assert_eq!(
                KeyRef::parse(text),
                Err(KeyRefError::NotAReference),
                "{text}"
            );
        }
    }

    #[test]
    fn a_keychain_reference_needs_the_wrybill_service_and_a_plain_name() {
        for text in [
            "keychain:",
            "keychain:wrybill",
            "keychain:wrybill/",
            "keychain:other/anthropic",
            "keychain:wrybill/Anthropic",
            "keychain:wrybill/my key",
            "keychain:wrybill/a/b",
        ] {
            assert_eq!(
                KeyRef::parse(text),
                Err(KeyRefError::BadKeychainReference),
                "{text}"
            );
        }
    }

    #[test]
    fn an_env_reference_needs_a_variable_name() {
        for text in ["env:", "env:1KEY", "env:MY KEY", "env:MY-KEY", "env:$KEY"] {
            assert_eq!(
                KeyRef::parse(text),
                Err(KeyRefError::BadEnvReference),
                "{text}"
            );
        }
    }

    #[test]
    fn key_names_use_lowercase_letters_digits_hyphens_and_underscores() {
        for name in ["anthropic", "brave", "openai-2", "my_key", "a"] {
            assert!(is_valid_key_name(name), "{name}");
        }
        for name in ["", "Anthropic", "my key", "a/b", "clé", &"x".repeat(65)] {
            assert!(!is_valid_key_name(name), "{name}");
        }
    }
}
