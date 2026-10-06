//! `wrybill keys set`, run in this process against a store that lives in
//! memory. No real keychain is touched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tracing::Level;
use wrybill_cli::keys::{self, FAILED, REFUSED, SAVED};
use wrybill_cli::logging;
use wrybill_config::{MemoryStore, Paths, SecretStore};

/// Something shaped like a key. No test may ever find it in what the command
/// prints or logs.
const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

/// What one run of the command left behind.
struct Run {
    status: u8,
    out: String,
    err: String,
    /// The whole log file, written at the most detailed level.
    log: String,
    /// The prompts the user was shown.
    prompts: Vec<String>,
}

impl Run {
    /// Fails the test if the key shows up anywhere it could be read back.
    fn never_shows_the_key(&self) {
        for (place, text) in [
            ("stdout", &self.out),
            ("stderr", &self.err),
            ("the log", &self.log),
        ] {
            assert!(!text.contains("MARKER"), "the key is in {place}:\n{text}");
        }
    }
}

/// A data folder for one test, inside Cargo's own temp folder.
fn data_folder(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("keys")
        .join(test)
        .join("wrybill-home");
    // Left over from an earlier run, if it's there at all.
    let _ = fs::remove_dir_all(&folder);
    folder
}

/// Runs `wrybill keys set <name>` with `typed` as what the user gives when
/// asked for the key, and `extra_arguments` things after the name.
fn keys_set(
    test: &str,
    name: &str,
    extra_arguments: usize,
    typed: io::Result<&str>,
    store: &dyn SecretStore,
) -> Run {
    let paths = Paths::resolve(Some(data_folder(test).into()), None).expect("paths");
    let (file, log_file) = logging::open(&paths, SystemTime::now()).expect("a log file");

    let mut typed = Some(typed.map(str::to_owned));
    let mut prompts = Vec::new();
    let mut read_key = |prompt: &str| {
        prompts.push(prompt.to_owned());
        typed.take().expect("the key is asked for once")
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());

    // Everything is logged, down to the finest level, so nothing can hide.
    let status = tracing::subscriber::with_default(logging::subscriber(file, Level::TRACE), || {
        keys::set(
            name,
            extra_arguments,
            &mut read_key,
            store,
            &mut out,
            &mut err,
        )
    });

    Run {
        status,
        out: String::from_utf8(out).expect("stdout is UTF-8"),
        err: String::from_utf8(err).expect("stderr is UTF-8"),
        log: fs::read_to_string(log_file).expect("the log file"),
        prompts,
    }
}

fn saved_key(store: &MemoryStore, name: &str) -> Option<String> {
    let key = store.read(name).expect("the test store");
    key.map(|key| key.expose().to_owned())
}

#[test]
fn a_key_for_a_model_provider_is_saved_and_needs_no_config_line() {
    let store = MemoryStore::new();

    let run = keys_set("model-provider", "anthropic", 0, Ok(MARKER), &store);

    assert_eq!(run.status, SAVED);
    assert_eq!(
        run.out,
        "Saved the key for anthropic in the test store.\n\
         Wrybill will now use it for every \"anthropic\" model. The config needs no line for it.\n"
    );
    assert_eq!(run.err, "");
    assert_eq!(
        run.prompts,
        ["Key for anthropic (hidden as you type or paste): "]
    );
    assert_eq!(saved_key(&store, "anthropic").as_deref(), Some(MARKER));
    run.never_shows_the_key();
}

#[test]
fn a_key_for_a_search_provider_is_saved_and_needs_no_config_line() {
    let store = MemoryStore::new();

    let run = keys_set("search-provider", "brave", 0, Ok(MARKER), &store);

    assert_eq!(run.status, SAVED);
    assert_eq!(
        run.out,
        "Saved the key for brave in the test store.\n\
         Wrybill will now use it for web search with \"brave\". The config needs no line for it.\n"
    );
    assert_eq!(saved_key(&store, "brave").as_deref(), Some(MARKER));
    run.never_shows_the_key();
}

#[test]
fn a_key_under_another_name_comes_with_the_line_to_add() {
    let store = MemoryStore::new();

    let run = keys_set("another-name", "claude-work", 0, Ok(MARKER), &store);

    assert_eq!(run.status, SAVED);
    assert_eq!(
        run.out,
        "Saved the key \"claude-work\" in the test store.\n\
         To use it, add this line to a model in config.toml:\n\
         \x20 api_key = \"keychain:wrybill/claude-work\"\n"
    );
    assert_eq!(
        run.prompts,
        ["Key to save as \"claude-work\" (hidden as you type or paste): "]
    );
    assert_eq!(saved_key(&store, "claude-work").as_deref(), Some(MARKER));
    run.never_shows_the_key();
}

#[test]
fn a_name_the_user_made_up_is_not_logged() {
    let store = MemoryStore::new();

    let run = keys_set("made-up-name", "claude-work", 0, Ok(MARKER), &store);

    assert!(run.log.contains("saved a key"), "{}", run.log);
    assert!(!run.log.contains("claude-work"), "{}", run.log);
}

#[test]
fn a_provider_name_is_logged_so_the_log_says_what_was_saved() {
    let store = MemoryStore::new();

    let run = keys_set("provider-name-logged", "openai", 0, Ok(MARKER), &store);

    assert!(run.log.contains("\"name\":\"openai\""), "{}", run.log);
}

#[test]
fn what_is_typed_is_tidied_before_it_is_saved() {
    let store = MemoryStore::new();
    // A byte-order mark, spaces and a line ending around the key.
    let typed = format!("\u{feff}  {MARKER} \r\n");

    let run = keys_set("tidied", "gemini", 0, Ok(&typed), &store);

    assert_eq!(run.status, SAVED);
    assert_eq!(saved_key(&store, "gemini").as_deref(), Some(MARKER));
}

#[test]
fn a_new_key_replaces_the_old_one() {
    let store = MemoryStore::new();

    keys_set("replace-first", "openrouter", 0, Ok("the-old-key"), &store);
    let run = keys_set("replace-second", "openrouter", 0, Ok(MARKER), &store);

    assert_eq!(run.status, SAVED);
    assert_eq!(saved_key(&store, "openrouter").as_deref(), Some(MARKER));
}

#[test]
fn a_key_on_the_command_line_is_refused_and_never_read() {
    let store = MemoryStore::new();

    let run = keys_set("on-the-command-line", "anthropic", 1, Ok(MARKER), &store);

    assert_eq!(run.status, REFUSED);
    assert_eq!(run.out, "");
    assert_eq!(
        run.err,
        "A key doesn't go on the command line. Anything typed there stays in your shell history and shows in the list of running programs.\n\
         \n\
         Run this on its own, then paste the key when asked:\n\
         \x20 wrybill keys set anthropic\n\
         \n\
         If what you typed just now was a real key, the safest thing is to replace it with a new one.\n"
    );
    assert!(run.prompts.is_empty(), "the key was asked for anyway");
    assert_eq!(saved_key(&store, "anthropic"), None);
    run.never_shows_the_key();
}

#[test]
fn a_key_typed_where_the_name_goes_is_refused_and_not_echoed() {
    let store = MemoryStore::new();
    let key_as_name = MARKER.to_uppercase();

    let run = keys_set("key-as-name", &key_as_name, 0, Ok(MARKER), &store);

    assert_eq!(run.status, REFUSED);
    assert_eq!(
        run.err,
        "That isn't a name a key can be saved under. A name uses lowercase letters, digits, - and _, such as anthropic, openai or brave.\n\
         \n\
         If you typed the key itself there, it's now in your shell history. Run `wrybill keys set <provider>` on its own and paste the key when asked.\n"
    );
    assert!(run.prompts.is_empty());
    run.never_shows_the_key();
}

#[test]
fn a_key_on_the_command_line_after_a_bad_name_shows_neither() {
    let store = MemoryStore::new();
    let key_as_name = MARKER.to_uppercase();

    let run = keys_set("bad-name-and-extra", &key_as_name, 2, Ok(MARKER), &store);

    assert_eq!(run.status, REFUSED);
    assert!(
        run.err.contains("wrybill keys set <provider>"),
        "{}",
        run.err
    );
    run.never_shows_the_key();
}

#[test]
fn searxng_has_no_key_to_save() {
    let store = MemoryStore::new();

    let run = keys_set("searxng", "searxng", 0, Ok(MARKER), &store);

    assert_eq!(run.status, REFUSED);
    assert_eq!(
        run.err,
        "SearXNG takes no key, so there's nothing to save.\n"
    );
    assert!(run.prompts.is_empty());
    assert_eq!(saved_key(&store, "searxng"), None);
}

#[test]
fn an_empty_key_is_not_saved() {
    let store = MemoryStore::new();

    for (test, typed) in [("empty", ""), ("only-spaces", "  \n")] {
        let run = keys_set(test, "anthropic", 0, Ok(typed), &store);

        assert_eq!(run.status, FAILED);
        assert_eq!(run.err, "No key was given, so nothing was saved.\n");
        assert_eq!(saved_key(&store, "anthropic"), None);
    }
}

#[test]
fn more_than_one_line_is_not_a_key() {
    let store = MemoryStore::new();
    let typed = format!("{MARKER}\nand-a-second-line");

    let run = keys_set("two-lines", "anthropic", 0, Ok(&typed), &store);

    assert_eq!(run.status, FAILED);
    assert_eq!(
        run.err,
        "A key is a single line of text, and this was more than that. Nothing was saved.\n"
    );
    assert_eq!(saved_key(&store, "anthropic"), None);
    run.never_shows_the_key();
}

#[test]
fn a_key_that_cannot_be_read_is_reported() {
    let store = MemoryStore::new();
    let broken = io::Error::new(io::ErrorKind::BrokenPipe, "the pipe closed");

    let run = keys_set("cannot-read", "anthropic", 0, Err(broken), &store);

    assert_eq!(run.status, FAILED);
    assert_eq!(
        run.err,
        "Wrybill couldn't read the key (the pipe closed). Nothing was saved.\n"
    );
}

#[test]
fn with_no_keychain_the_key_is_not_saved_and_the_env_way_is_offered() {
    let store = MemoryStore::unreachable();

    let run = keys_set("no-keychain", "anthropic", 0, Ok(MARKER), &store);

    assert_eq!(run.status, FAILED);
    assert_eq!(run.out, "");
    assert_eq!(
        run.err,
        "Wrybill couldn't reach the test store, so the key wasn't saved.\n\
         What it said: this test store has no keychain\n\
         \n\
         On a machine with no keychain to use, such as a server, a container or an SSH session, keep the key in an environment variable and point the config at it:\n\
         \x20 api_key = \"env:ANTHROPIC_API_KEY\"\n"
    );
    run.never_shows_the_key();
}

#[test]
fn every_way_a_key_is_not_saved_leaves_a_reason_in_the_log() {
    let store = MemoryStore::new();

    let refused = keys_set("log-refused", "anthropic", 1, Ok(MARKER), &store);
    let empty = keys_set("log-empty", "anthropic", 0, Ok(""), &store);
    let unreachable = keys_set(
        "log-unreachable",
        "anthropic",
        0,
        Ok(MARKER),
        &MemoryStore::unreachable(),
    );

    for (run, reason) in [
        (&refused, "something followed the name on the command line"),
        (&empty, "no key was given"),
        (&unreachable, "the keychain didn't take the key"),
    ] {
        assert!(run.log.contains("nothing was saved"), "{}", run.log);
        assert!(run.log.contains(reason), "{}", run.log);
        run.never_shows_the_key();
    }
}
