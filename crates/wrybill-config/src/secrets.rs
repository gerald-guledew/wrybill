//! Where API keys are kept (spec 11.6).
//!
//! Real keys live in the OS keychain: the macOS Keychain, Windows Credential
//! Manager, or the Secret Service on Linux. Tests use [`MemoryStore`], so
//! they never touch a real keychain.
//!
//! Nothing here can print a key. A [`Secret`] hides itself when it's
//! formatted, and no error holds any text taken from one.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::sync::Mutex;

use crate::keyref::{KEYCHAIN_SERVICE, KeyRef, is_valid_key_name};

/// A name that no key is ever saved under. Looking it up shows whether the
/// keychain answers at all.
const CHECK_NAME: &str = "wrybill-keychain-check";

/// An API key.
///
/// It can't be printed or logged by accident: its `Debug` text hides it, it
/// has no `Display`, and getting at the key takes a deliberate call to
/// [`Secret::expose`].
pub struct Secret(String);

impl Secret {
    /// Wraps a key.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The key itself. Only call this where the key is really needed, and
    /// never to print or log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(hidden)")
    }
}

/// Why a secret store couldn't do what it was asked.
///
/// The text inside comes from the OS, never from a key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// No keychain can be reached from here: a server or container with no
    /// keychain, or one that's locked away from this session.
    #[error("no keychain can be reached ({0})")]
    Unreachable(String),
    /// The keychain was reached, but it refused or failed.
    #[error("the keychain reported a problem ({0})")]
    Failed(String),
    /// The name isn't one a key can be saved under.
    #[error("a key's name uses lowercase letters, digits, - and _")]
    BadName,
}

/// Somewhere keys can be saved under a name and found again.
pub trait SecretStore {
    /// What to call this store in a message, such as "the macOS Keychain".
    fn name(&self) -> &'static str;

    /// Checks that the store can be used on this machine at all.
    fn check(&self) -> Result<(), StoreError>;

    /// Saves a key under a name, replacing any key already saved there.
    fn save(&self, name: &str, key: &Secret) -> Result<(), StoreError>;

    /// Whether a key is saved under this name. The key itself isn't read, so
    /// asking can't set off a prompt for it.
    fn has(&self, name: &str) -> Result<bool, StoreError>;

    /// Reads the key saved under this name, if there is one.
    fn read(&self, name: &str) -> Result<Option<Secret>, StoreError>;

    /// Removes the key saved under this name. It's fine if there isn't one.
    fn remove(&self, name: &str) -> Result<(), StoreError>;
}

/// What a key reference points at right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStatus {
    /// There's a key where the reference points.
    Found,
    /// Nothing is there yet.
    Missing,
    /// The keychain couldn't be asked.
    Unavailable(StoreError),
}

/// Finds out whether a key reference resolves, without reading the key.
///
/// `env_var` looks up an environment variable. It's handed in so tests
/// never touch the real environment.
pub fn key_status(
    reference: &KeyRef,
    store: &dyn SecretStore,
    env_var: impl Fn(&str) -> Option<OsString>,
) -> KeyStatus {
    match reference {
        KeyRef::Keychain { name } => match store.has(name) {
            Ok(true) => KeyStatus::Found,
            Ok(false) => KeyStatus::Missing,
            Err(error) => KeyStatus::Unavailable(error),
        },
        KeyRef::Env { variable } => {
            if env_var(variable).is_some_and(|value| !value.is_empty()) {
                KeyStatus::Found
            } else {
                KeyStatus::Missing
            }
        }
    }
}

/// The OS keychain: the macOS Keychain, Windows Credential Manager, or the
/// Secret Service on Linux.
///
/// Every Wrybill key is saved under the service `wrybill`, with the key's
/// name as the account.
#[derive(Debug, Clone, Copy, Default)]
pub struct Keychain;

impl Keychain {
    /// Makes sure the OS keychain is set up for this process.
    fn ready() -> Result<(), StoreError> {
        match keyring::Entry::store_status() {
            Ok(()) => Ok(()),
            // Whatever went wrong while connecting, the keychain can't be used.
            Err(error) => Err(StoreError::Unreachable(detail(error))),
        }
    }

    fn entry(name: &str) -> Result<keyring::Entry, StoreError> {
        if !is_valid_key_name(name) {
            return Err(StoreError::BadName);
        }
        Self::ready()?;
        keyring::Entry::new(KEYCHAIN_SERVICE, name).map_err(|error| store_error(&error))
    }

    /// Looks for a key by its attributes only.
    ///
    /// On macOS this asks for the item's attributes and never its data, so
    /// it can't set off the prompt that reading a key from a different
    /// build does. On Linux it searches the Secret Service by attribute,
    /// which doesn't read the secret either. `user_attribute` is what the
    /// store calls the account.
    #[cfg(not(windows))]
    fn find(name: &str, user_attribute: &str) -> Result<bool, StoreError> {
        use std::collections::HashMap;

        let wanted = HashMap::from([("service", KEYCHAIN_SERVICE), (user_attribute, name)]);
        keyring_core::Entry::search(&wanted)
            .map(|found| !found.is_empty())
            .map_err(|error| store_error(&error))
    }
}

impl SecretStore for Keychain {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "the macOS Keychain"
        } else if cfg!(windows) {
            "Windows Credential Manager"
        } else {
            "the Secret Service keyring"
        }
    }

    fn check(&self) -> Result<(), StoreError> {
        self.has(CHECK_NAME).map(|_| ())
    }

    fn save(&self, name: &str, key: &Secret) -> Result<(), StoreError> {
        Self::entry(name)?
            .set_password(key.expose())
            .map_err(|error| store_error(&error))
    }

    #[cfg(target_os = "macos")]
    fn has(&self, name: &str) -> Result<bool, StoreError> {
        Self::entry(name)?;
        Self::find(name, "user")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn has(&self, name: &str) -> Result<bool, StoreError> {
        Self::entry(name)?;
        Self::find(name, "username")
    }

    // Windows has no prompts to avoid, and its search would list every
    // credential the user has. Asking for this one credential's attributes
    // is the narrowest lookup there.
    #[cfg(windows)]
    fn has(&self, name: &str) -> Result<bool, StoreError> {
        match Self::entry(name)?.inner.get_attributes() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(store_error(&error)),
        }
    }

    fn read(&self, name: &str) -> Result<Option<Secret>, StoreError> {
        match Self::entry(name)?.get_password() {
            Ok(key) => Ok(Some(Secret::new(key))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(store_error(&error)),
        }
    }

    fn remove(&self, name: &str) -> Result<(), StoreError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(&error)),
        }
    }
}

/// Turns the keychain library's error into ours.
///
/// The library's error can carry the bytes of a key, so it's never passed
/// on or formatted whole. Only text that comes from the OS is kept.
fn store_error(error: &keyring::Error) -> StoreError {
    use keyring::Error;

    match error {
        Error::NoStorageAccess(_) | Error::NoDefaultStore => StoreError::Unreachable(detail(error)),
        _ => StoreError::Failed(detail(error)),
    }
}

/// A short description of what went wrong, safe to show.
fn detail(error: &keyring::Error) -> String {
    use keyring::Error;

    match error {
        Error::PlatformFailure(platform) | Error::NoStorageAccess(platform) => platform.to_string(),
        Error::NoEntry => "there's no key under that name".to_owned(),
        Error::BadEncoding(_) | Error::BadDataFormat(..) => {
            "what's saved there isn't readable text".to_owned()
        }
        Error::TooLong(what, limit) => {
            format!("the {what} is longer than the {limit} characters this keychain allows")
        }
        Error::Invalid(what, why) => format!("the {what} {why}"),
        Error::Ambiguous(found) => {
            format!("{} keys are saved under that name", found.len())
        }
        Error::NoDefaultStore => "no keychain is set up".to_owned(),
        Error::NotSupportedByStore(what) => what.clone(),
        // The library can add kinds of error. One this code doesn't know
        // isn't formatted, in case it carries something it shouldn't show.
        _ => "an error Wrybill doesn't know about".to_owned(),
    }
}

/// A secret store that lives in memory, for tests.
///
/// Nothing in it is kept or protected, so it's never used for real keys.
#[derive(Default)]
pub struct MemoryStore {
    keys: Mutex<BTreeMap<String, String>>,
    unreachable: bool,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A store that stands in for a machine with no keychain: every call
    /// fails as unreachable.
    pub fn unreachable() -> Self {
        Self {
            keys: Mutex::default(),
            unreachable: true,
        }
    }

    /// The keys, once the name has been checked and the store is reachable.
    fn unlocked(
        &self,
        name: &str,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, String>>, StoreError> {
        if !is_valid_key_name(name) {
            return Err(StoreError::BadName);
        }
        if self.unreachable {
            return Err(StoreError::Unreachable(
                "this test store has no keychain".to_owned(),
            ));
        }
        self.keys
            .lock()
            .map_err(|_| StoreError::Failed("the test store is broken".to_owned()))
    }
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The names only. Even a test key isn't printed.
        let names: Vec<String> = match self.keys.lock() {
            Ok(keys) => keys.keys().cloned().collect(),
            Err(_) => Vec::new(),
        };
        f.debug_struct("MemoryStore")
            .field("names", &names)
            .field("unreachable", &self.unreachable)
            .finish()
    }
}

impl SecretStore for MemoryStore {
    fn name(&self) -> &'static str {
        "the test store"
    }

    fn check(&self) -> Result<(), StoreError> {
        self.unlocked(CHECK_NAME).map(|_| ())
    }

    fn save(&self, name: &str, key: &Secret) -> Result<(), StoreError> {
        self.unlocked(name)?
            .insert(name.to_owned(), key.expose().to_owned());
        Ok(())
    }

    fn has(&self, name: &str) -> Result<bool, StoreError> {
        Ok(self.unlocked(name)?.contains_key(name))
    }

    fn read(&self, name: &str) -> Result<Option<Secret>, StoreError> {
        Ok(self.unlocked(name)?.get(name).map(Secret::new))
    }

    fn remove(&self, name: &str) -> Result<(), StoreError> {
        self.unlocked(name)?.remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{KeyStatus, MemoryStore, Secret, SecretStore, StoreError, key_status};
    use crate::keyref::KeyRef;

    /// Something shaped like a key. No test may ever find it printed.
    const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    #[test]
    fn a_secret_hides_itself_when_it_is_formatted() {
        let secret = Secret::new(MARKER);

        assert_eq!(format!("{secret:?}"), "Secret(hidden)");
        assert_eq!(format!("{:?}", Some(&secret)), "Some(Secret(hidden))");
        assert_eq!(secret.expose(), MARKER);
    }

    #[test]
    fn a_saved_key_can_be_found_read_replaced_and_removed() {
        let store = MemoryStore::new();
        assert_eq!(store.has("anthropic"), Ok(false));
        assert!(store.read("anthropic").expect("a store").is_none());

        store
            .save("anthropic", &Secret::new("first"))
            .expect("saved");
        assert_eq!(store.has("anthropic"), Ok(true));
        assert_eq!(store.has("openai"), Ok(false));

        store
            .save("anthropic", &Secret::new("second"))
            .expect("saved");
        let read = store.read("anthropic").expect("a store").expect("a key");
        assert_eq!(read.expose(), "second");

        store.remove("anthropic").expect("removed");
        assert_eq!(store.has("anthropic"), Ok(false));
        store.remove("anthropic").expect("removing nothing is fine");
    }

    #[test]
    fn a_key_cannot_be_saved_under_a_name_that_is_not_plain() {
        let store = MemoryStore::new();

        for name in ["", "Anthropic", "my key", MARKER.to_uppercase().as_str()] {
            assert_eq!(
                store.save(name, &Secret::new(MARKER)),
                Err(StoreError::BadName),
                "{name}"
            );
        }
    }

    #[test]
    fn a_store_with_no_keychain_fails_every_call_as_unreachable() {
        let store = MemoryStore::unreachable();

        assert!(matches!(store.check(), Err(StoreError::Unreachable(_))));
        assert!(matches!(
            store.save("anthropic", &Secret::new(MARKER)),
            Err(StoreError::Unreachable(_))
        ));
        assert!(matches!(
            store.has("anthropic"),
            Err(StoreError::Unreachable(_))
        ));
    }

    #[test]
    fn the_test_store_never_prints_a_key() {
        let store = MemoryStore::new();
        store
            .save("anthropic", &Secret::new(MARKER))
            .expect("saved");

        let printed = format!("{store:?}");

        assert!(printed.contains("anthropic"), "{printed}");
        assert!(!printed.contains("MARKER"), "{printed}");
    }

    #[test]
    fn a_keychain_reference_is_found_once_its_key_is_saved() {
        let store = MemoryStore::new();
        let reference = KeyRef::keychain("anthropic").expect("a valid name");

        assert_eq!(key_status(&reference, &store, no_env), KeyStatus::Missing);

        store
            .save("anthropic", &Secret::new(MARKER))
            .expect("saved");
        assert_eq!(key_status(&reference, &store, no_env), KeyStatus::Found);
    }

    #[test]
    fn a_keychain_reference_is_unavailable_when_no_keychain_can_be_reached() {
        let store = MemoryStore::unreachable();
        let reference = KeyRef::keychain("anthropic").expect("a valid name");

        assert!(matches!(
            key_status(&reference, &store, no_env),
            KeyStatus::Unavailable(StoreError::Unreachable(_))
        ));
    }

    #[test]
    fn an_env_reference_is_found_when_the_variable_is_set_to_something() {
        let store = MemoryStore::new();
        let reference = KeyRef::parse("env:MY_API_KEY").expect("a valid reference");
        let env = |value: &'static str| {
            move |name: &str| (name == "MY_API_KEY").then(|| OsString::from(value))
        };

        assert_eq!(
            key_status(&reference, &store, env(MARKER)),
            KeyStatus::Found
        );
        assert_eq!(key_status(&reference, &store, env("")), KeyStatus::Missing);
        assert_eq!(key_status(&reference, &store, no_env), KeyStatus::Missing);
    }
}
