//! Wrybill's config: loading and validation, paths and the keychain
//! (spec section 13).
//!
//! - [`Paths`] works out where Wrybill keeps its data on this machine.
//! - [`load`] reads `config.toml` and checks it against spec 13.3. A missing
//!   file isn't an error: Wrybill starts on built-in defaults.
//! - [`Config`] is the result: every setting, with its default filled in.
//! - [`Keychain`] keeps API keys in the OS keychain. The config only ever
//!   holds a [`KeyRef`] that points at one (spec 11.6).

mod check;
mod keyref;
mod load;
mod model;
mod paths;
mod problem;
mod reader;
mod secrets;
mod validate;

pub use url::Url;

pub use crate::keyref::{KEYCHAIN_SERVICE, KeyRef, KeyRefError, is_valid_key_name};
pub use crate::load::{Invalid, LoadError, Loaded, Source, Valid, load, parse};
pub use crate::model::{
    Autonomy, Config, CostTier, Defaults, KeyUse, Limits, LocalToCloud, Locality, McpServer,
    McpTransport, McpTrust, Model, Provider, Role, Routing, Safety, Search, SearchProvider,
    Storage, ToolCalling, key_use,
};
pub use crate::paths::{Paths, PathsError, expand_tilde};
pub use crate::problem::Problem;
pub use crate::secrets::{
    KeyStatus, Keychain, MemoryStore, Secret, SecretStore, StoreError, key_status,
};
