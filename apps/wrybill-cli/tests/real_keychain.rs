//! `wrybill keys set` against the real OS keychain.
//!
//! It's ignored by default, because it writes to the keychain of whoever
//! runs it. Run it on purpose with:
//!
//! ```sh
//! cargo test -p wrybill-cli --test real_keychain -- --ignored
//! ```
//!
//! It saves a dummy value under a name of its own, and removes it again.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use wrybill_config::{Keychain, SecretStore};

/// Something shaped like a key. It must never show up in what the command
/// prints or logs.
const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

/// Removes the test's entry when the test ends, whether it passed or not.
struct Cleanup<'a>(&'a str);

impl Drop for Cleanup<'_> {
    // On macOS an entry can only be removed without a prompt by the program
    // that made it, which here was `wrybill` and not this test. Apple's own
    // `security` tool is allowed to, so it does the tidying up.
    #[cfg(target_os = "macos")]
    fn drop(&mut self) {
        let _ = Command::new("security")
            .args(["delete-generic-password", "-s", "wrybill", "-a", self.0])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(target_os = "macos"))]
    fn drop(&mut self) {
        let _ = Keychain.remove(self.0);
    }
}

#[test]
#[ignore = "writes a dummy entry to the real OS keychain"]
fn real_keychain_keys_set_saves_a_piped_key_and_shows_it_nowhere() {
    let name = format!("wrybill-selftest-cli-{}", std::process::id());
    let _cleanup = Cleanup(&name);
    let data_folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("real-keychain")
        .join("wrybill-home");
    let _ = fs::remove_dir_all(&data_folder);

    let mut child = Command::new(env!("CARGO_BIN_EXE_wrybill"))
        .args(["keys", "set", &name])
        .env("WRYBILL_HOME", &data_folder)
        .env("WRYBILL_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the wrybill binary should start");
    child
        .stdin
        .take()
        .expect("a pipe to the command")
        .write_all(format!("{MARKER}\n").as_bytes())
        .expect("the key goes down the pipe");
    let output = child.wait_with_output().expect("the command finishes");

    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(
        stdout.contains(&format!("api_key = \"keychain:wrybill/{name}\"")),
        "{stdout}"
    );

    // The key is nowhere it could be read back.
    let log: String = fs::read_dir(data_folder.join("logs"))
        .expect("the logs folder")
        .map(|file| fs::read_to_string(file.expect("a log file").path()).expect("log text"))
        .collect();
    assert!(log.contains("saved a key"), "{log}");
    for (place, text) in [("stdout", &stdout), ("stderr", &stderr), ("the log", &log)] {
        assert!(!text.contains("MARKER"), "the key is in {place}:\n{text}");
    }

    // Only Wrybill's own events are in the log. On Linux the keychain client
    // writes events of its own at this level, and none of them may get in.
    for line in log.lines() {
        let event: serde_json::Value = serde_json::from_str(line).expect("each log line is JSON");
        let target = event["target"].as_str().expect("where the event came from");
        assert!(
            target == "wrybill" || target.starts_with("wrybill_"),
            "an event from another crate is in the log:\n{line}"
        );
    }

    // Another program (this test) can tell the key is there without reading
    // it. That's what `wrybill doctor` does, so it mustn't prompt.
    assert_eq!(Keychain.has(&name), Ok(true));
}
