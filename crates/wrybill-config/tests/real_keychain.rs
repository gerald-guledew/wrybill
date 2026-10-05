//! A round trip through the real OS keychain.
//!
//! It's ignored by default, because it writes to the keychain of whoever
//! runs it. Run it on purpose with:
//!
//! ```sh
//! cargo test -p wrybill-config --test real_keychain -- --ignored
//! ```
//!
//! It saves a dummy value under a name of its own, and removes it again.

use wrybill_config::{Keychain, Secret, SecretStore};

/// Removes the test's entry when the test ends, whether it passed or not.
struct Cleanup<'a>(&'a str);

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = Keychain.remove(self.0);
    }
}

#[test]
#[ignore = "writes a dummy entry to the real OS keychain"]
fn real_keychain_round_trip() {
    let store = Keychain;
    let name = format!("wrybill-selftest-{}", std::process::id());
    let _cleanup = Cleanup(&name);

    store
        .check()
        .unwrap_or_else(|error| panic!("{} should be reachable: {error}", store.name()));
    assert_eq!(store.has(&name), Ok(false));
    assert!(store.read(&name).expect("a lookup").is_none());

    store
        .save(&name, &Secret::new("not-a-real-key-1"))
        .expect("the first save");
    assert_eq!(store.has(&name), Ok(true));
    let read = store.read(&name).expect("a lookup").expect("the saved key");
    assert_eq!(read.expose(), "not-a-real-key-1");

    // Saving again replaces the key.
    store
        .save(&name, &Secret::new("not-a-real-key-2"))
        .expect("the second save");
    let read = store.read(&name).expect("a lookup").expect("the saved key");
    assert_eq!(read.expose(), "not-a-real-key-2");

    store.remove(&name).expect("removing the key");
    assert_eq!(store.has(&name), Ok(false));
    store.remove(&name).expect("removing nothing is fine");
}
