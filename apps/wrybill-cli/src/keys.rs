//! `wrybill keys set <provider>`: saves an API key in the OS keychain
//! (spec 7.5 and 13.1).
//!
//! The key is typed at a hidden prompt or piped in. It's never taken from the
//! command line, never printed and never logged.

use std::io::{self, Write};

use wrybill_config::{
    KeyRef, KeyUse, SearchProvider, Secret, SecretStore, StoreError, is_valid_key_name, key_use,
};

/// The exit status when the key was saved.
pub const SAVED: u8 = 0;
/// The exit status when the key couldn't be read or saved.
pub const FAILED: u8 = 1;
/// The exit status when the command was used in a way it doesn't allow.
pub const REFUSED: u8 = 2;

/// Why a key wasn't saved.
struct NotSaved {
    /// The exit status.
    status: u8,
    /// A few fixed words for the log. Never anything the user typed.
    reason: &'static str,
    /// What the keychain said, when it was the keychain that failed.
    detail: Option<String>,
    /// What to tell the user.
    message: String,
}

impl NotSaved {
    fn refused(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: REFUSED,
            reason,
            detail: None,
            message: message.into(),
        }
    }

    fn failed(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: FAILED,
            reason,
            detail: None,
            message: message.into(),
        }
    }
}

/// Saves the key for `name` in `store`, and says what happens next.
///
/// `extra_arguments` is how many arguments followed the name. Only the count
/// is passed in, so what they say can't be shown or logged from here.
/// `read_key` asks for the key: it's given the prompt to show.
///
/// The result goes to `out`, anything that went wrong goes to `err`, and the
/// exit status comes back.
pub fn set(
    name: &str,
    extra_arguments: usize,
    read_key: &mut dyn FnMut(&str) -> io::Result<String>,
    store: &dyn SecretStore,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    tracing::info!(command = "keys set", "started");
    // If the terminal has gone away there's nobody to tell, so a failed
    // write is let go.
    match save(name, extra_arguments, read_key, store) {
        Ok(message) => {
            let _ = writeln!(out, "{message}");
            SAVED
        }
        Err(not_saved) => {
            tracing::warn!(
                command = "keys set",
                reason = not_saved.reason,
                detail = not_saved.detail.as_deref(),
                "nothing was saved"
            );
            let _ = writeln!(err, "{}", not_saved.message);
            not_saved.status
        }
    }
}

fn save(
    name: &str,
    extra_arguments: usize,
    read_key: &mut dyn FnMut(&str) -> io::Result<String>,
    store: &dyn SecretStore,
) -> Result<String, NotSaved> {
    let name_is_valid = is_valid_key_name(name);

    if extra_arguments > 0 {
        let shown = if name_is_valid { name } else { "<provider>" };
        return Err(NotSaved::refused(
            "something followed the name on the command line",
            format!(
                "A key doesn't go on the command line. Anything typed there stays in your shell history and shows in the list of running programs.\n\
                 \n\
                 Run this on its own, then paste the key when asked:\n\
                 \x20 wrybill keys set {shown}\n\
                 \n\
                 If what you typed just now was a real key, the safest thing is to replace it with a new one."
            ),
        ));
    }

    if !name_is_valid {
        // The name isn't shown or logged: it may be a key typed in its place.
        return Err(NotSaved::refused(
            "the name isn't one a key can have",
            "That isn't a name a key can be saved under. A name uses lowercase letters, digits, - and _, such as anthropic, openai or brave.\n\
             \n\
             If you typed the key itself there, it's now in your shell history. Run `wrybill keys set <provider>` on its own and paste the key when asked.",
        ));
    }

    if name == SearchProvider::Searxng.as_str() {
        return Err(NotSaved::refused(
            "searxng takes no key",
            "SearXNG takes no key, so there's nothing to save.",
        ));
    }

    let used_by = key_use(name);
    let prompt = match used_by {
        KeyUse::Models(_) | KeyUse::Search(_) => {
            format!("Key for {name} (hidden as you type or paste): ")
        }
        KeyUse::ByReference => {
            format!("Key to save as \"{name}\" (hidden as you type or paste): ")
        }
    };
    let typed = read_key(&prompt).map_err(|error| NotSaved {
        detail: Some(error.to_string()),
        ..NotSaved::failed(
            "the key couldn't be read",
            format!("Wrybill couldn't read the key ({error}). Nothing was saved."),
        )
    })?;
    let key = clean(&typed)?;

    store.save(name, &key).map_err(|error| NotSaved {
        detail: Some(error.to_string()),
        ..NotSaved::failed(
            "the keychain didn't take the key",
            not_saved_message(name, store.name(), &error),
        )
    })?;

    // A name the user made up isn't logged, in case it's a key typed in the
    // wrong place.
    let logged_name = match used_by {
        KeyUse::ByReference => "(a name of the user's choice)",
        KeyUse::Models(_) | KeyUse::Search(_) => name,
    };
    tracing::info!(command = "keys set", name = logged_name, "saved a key");

    let stored_in = store.name();
    Ok(match used_by {
        KeyUse::Models(_) => format!(
            "Saved the key for {name} in {stored_in}.\n\
             Wrybill will now use it for every \"{name}\" model. The config needs no line for it."
        ),
        KeyUse::Search(_) => format!(
            "Saved the key for {name} in {stored_in}.\n\
             Wrybill will now use it for web search with \"{name}\". The config needs no line for it."
        ),
        KeyUse::ByReference => {
            let reference = KeyRef::Keychain {
                name: name.to_owned(),
            };
            format!(
                "Saved the key \"{name}\" in {stored_in}.\n\
                 To use it, add this line to a model in config.toml:\n\
                 \x20 api_key = \"{reference}\""
            )
        }
    })
}

/// Tidies what was typed or piped in, and refuses anything that can't be a key.
fn clean(typed: &str) -> Result<Secret, NotSaved> {
    // Some Windows tools put a byte-order mark at the start of what they pipe.
    let key = typed.trim_start_matches('\u{feff}').trim();
    if key.is_empty() {
        return Err(NotSaved::failed(
            "no key was given",
            "No key was given, so nothing was saved.",
        ));
    }
    if key.chars().any(char::is_control) {
        return Err(NotSaved::failed(
            "what was given isn't a single line",
            "A key is a single line of text, and this was more than that. Nothing was saved.",
        ));
    }
    Ok(Secret::new(key))
}

/// What to say when the store wouldn't take the key.
fn not_saved_message(name: &str, store: &str, error: &StoreError) -> String {
    let what_happened = match error {
        StoreError::Unreachable(detail) => format!(
            "Wrybill couldn't reach {store}, so the key wasn't saved.\n\
             What it said: {detail}"
        ),
        StoreError::Failed(detail) => format!(
            "{} couldn't save the key.\n\
             What it said: {detail}",
            capitalised(store)
        ),
        StoreError::BadName => format!("{} couldn't save the key.", capitalised(store)),
    };
    let variable = format!("{}_API_KEY", name.to_ascii_uppercase().replace('-', "_"));
    format!(
        "{what_happened}\n\
         \n\
         On a machine with no keychain to use, such as a server, a container or an SSH session, keep the key in an environment variable and point the config at it:\n\
         \x20 api_key = \"env:{variable}\""
    )
}

fn capitalised(text: &str) -> String {
    let mut letters = text.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}
