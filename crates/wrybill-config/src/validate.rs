//! Checks each key of a parsed config file against spec 13.3, then builds
//! the typed [`Config`] with every default filled in.
//!
//! Reading and building are separate on purpose. Reading goes through the
//! whole file and records every problem. Building only happens when there
//! were none.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

use toml::de::DeTable;
use url::Url;

use crate::keyref::{KeyRef, KeyRefError, is_valid_variable_name};
use crate::model::{
    Autonomy, Config, CostTier, Defaults, Limits, LocalToCloud, Locality, McpServer, McpTransport,
    McpTrust, Model, Provider, Role, Routing, SEARXNG_DEFAULT_ENDPOINT, Safety, Search,
    SearchProvider, Storage, ToolCalling,
};
use crate::paths::expand_tilde;
use crate::reader::{Amount, Item, Place, Report, Section, quoted_list, shown_word};

/// The file as read: every value that passed its own checks.
pub(crate) struct Draft {
    defaults: Defaults,
    limits: Limits,
    routing: Routing,
    search: Option<SearchDraft>,
    safety: Safety,
    storage: Storage,
    models: Vec<ModelDraft>,
    mcp_servers: Vec<McpDraft>,
}

struct SearchDraft {
    provider: Option<SearchProvider>,
    api_key: Option<KeyRef>,
    endpoint: Option<Url>,
}

struct ModelDraft {
    id: Option<String>,
    provider: Option<Provider>,
    model: Option<String>,
    roles: Option<Vec<Role>>,
    api_key: Option<KeyRef>,
    endpoint: Option<Url>,
    locality: Option<Locality>,
    tools: Option<ToolCalling>,
    vision: Option<bool>,
    context_window: Option<u64>,
    min_free_ram_gb: Option<f64>,
    trusted_for_private: bool,
    cost_tier: Option<CostTier>,
    price_in_usd_per_mtok: Option<f64>,
    price_out_usd_per_mtok: Option<f64>,
    enabled: bool,
    notes: Option<String>,
}

struct McpDraft {
    id: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    url: Option<Url>,
    trust: McpTrust,
    pass_env: Vec<String>,
}

/// Reads the whole file, recording every problem in `report`.
pub(crate) fn read(table: &DeTable<'_>, user_home: Option<&Path>, report: &mut Report) -> Draft {
    let mut top = Section::new(table, Place::Top);

    if !top.has("version") {
        report.error(
            None,
            "version",
            "is missing. Put `version = 1` at the top of the file.",
        );
    }
    top.exactly(
        "version",
        1,
        "must be 1, the only config version this Wrybill reads.",
        report,
    );

    let defaults = match top.section("defaults", report) {
        Some(found) => read_defaults(found.value, report),
        None => Defaults::default(),
    };
    let limits = match top.section("limits", report) {
        Some(found) => read_limits(found.value, report),
        None => Limits::default(),
    };
    let routing = match top.section("routing", report) {
        Some(found) => read_routing(found.value, report),
        None => Routing::default(),
    };
    let search = top
        .section("search", report)
        .map(|found| read_search(found.value, report));
    let safety = match top.section("safety", report) {
        Some(found) => read_safety(found.value, user_home, report),
        None => Safety::defaults(user_home),
    };
    let storage = match top.section("storage", report) {
        Some(found) => read_storage(found.value, report),
        None => Storage::default(),
    };
    let models = top
        .entries("models", report)
        .into_iter()
        .map(|found| read_model(found.value, found.line, report))
        .collect();
    let mcp_servers = top
        .entries("mcp_servers", report)
        .into_iter()
        .map(|found| read_mcp_server(found.value, found.line, report))
        .collect();

    top.finish(report);

    Draft {
        defaults,
        limits,
        routing,
        search,
        safety,
        storage,
        models,
        mcp_servers,
    }
}

fn read_defaults(mut section: Section<'_>, report: &mut Report) -> Defaults {
    let mut model_id = |key| {
        section
            .filled_text(key, report)
            .map(|found| found.value.to_owned())
    };
    let planner = model_id("planner");
    let worker = model_id("worker");
    let summariser = model_id("summariser");
    let autonomy = section
        .choice("autonomy", &Autonomy::CHOICES, report)
        .map(|found| found.value)
        .unwrap_or_default();
    section.finish(report);

    Defaults {
        planner,
        worker,
        summariser,
        autonomy,
    }
}

fn read_limits(mut section: Section<'_>, report: &mut Report) -> Limits {
    let default = Limits::default();
    let mut count = |key, fallback: u32| {
        section
            .whole(key, 1, u64::from(u32::MAX), report)
            .and_then(|found| u32::try_from(found.value).ok())
            .unwrap_or(fallback)
    };
    let max_parallel_cloud_helpers = count(
        "max_parallel_cloud_helpers",
        default.max_parallel_cloud_helpers,
    );
    let max_parallel_calls_per_local_server = count(
        "max_parallel_calls_per_local_server",
        default.max_parallel_calls_per_local_server,
    );
    let max_steps_per_task = count("max_steps_per_task", default.max_steps_per_task);
    let max_minutes_per_task = count("max_minutes_per_task", default.max_minutes_per_task);

    section.exactly(
        "max_helper_depth",
        1,
        "must be 1 in this version of Wrybill.",
        report,
    );
    let max_tokens_per_task = section
        .whole("max_tokens_per_task", 1, i64::MAX.unsigned_abs(), report)
        .map_or(default.max_tokens_per_task, |found| found.value);
    let max_spend_usd_per_task = section
        .amount("max_spend_usd_per_task", Amount::AboveZero, report)
        .map_or(default.max_spend_usd_per_task, |found| found.value);
    section.finish(report);

    Limits {
        max_parallel_cloud_helpers,
        max_parallel_calls_per_local_server,
        max_helper_depth: default.max_helper_depth,
        max_steps_per_task,
        max_minutes_per_task,
        max_tokens_per_task,
        max_spend_usd_per_task,
    }
}

fn read_routing(mut section: Section<'_>, report: &mut Report) -> Routing {
    let default = Routing::default();
    let allow_cloud = section
        .flag("allow_cloud", report)
        .map_or(default.allow_cloud, |found| found.value);
    let local_to_cloud = section
        .choice("local_to_cloud", &LocalToCloud::CHOICES, report)
        .map_or(default.local_to_cloud, |found| found.value);
    section.finish(report);

    Routing {
        allow_cloud,
        local_to_cloud,
    }
}

fn read_search(mut section: Section<'_>, report: &mut Report) -> SearchDraft {
    let provider = section
        .choice("provider", &SearchProvider::CHOICES, report)
        .map(|found| found.value);
    let key_name = provider
        .filter(|provider| provider.takes_a_key())
        .map(SearchProvider::as_str);
    let api_key = read_key_ref(&mut section, key_name, report);
    let endpoint = read_url(&mut section, "endpoint", report);
    section.finish(report);

    SearchDraft {
        provider,
        api_key,
        endpoint,
    }
}

fn read_safety(mut section: Section<'_>, user_home: Option<&Path>, report: &mut Report) -> Safety {
    let default = Safety::defaults(user_home);

    let workspace_roots = match section.texts("workspace_roots", report) {
        Some(found) => read_workspace_roots(&section, &found.value.items, user_home, report),
        None => default.workspace_roots,
    };
    let trusted_install_domains = match section.texts("trusted_install_domains", report) {
        Some(found) => read_list(
            &section,
            "trusted_install_domains",
            &found.value.items,
            is_domain_name,
            "must be a domain name on its own, such as nodejs.org, with no https:// and no path.",
            report,
        ),
        None => default.trusted_install_domains,
    };
    let dev_hosts = match section.texts("dev_hosts", report) {
        Some(found) => read_list(
            &section,
            "dev_hosts",
            &found.value.items,
            is_host_pattern,
            "must be a host name, an IP address, or a pattern such as *.test.",
            report,
        ),
        None => default.dev_hosts,
    };
    let warn_on_unsupported_os = section
        .flag("warn_on_unsupported_os", report)
        .map_or(default.warn_on_unsupported_os, |found| found.value);
    section.finish(report);

    Safety {
        workspace_roots,
        trusted_install_domains,
        dev_hosts,
        warn_on_unsupported_os,
    }
}

/// Expands `~` in each folder and checks that the result is a full path.
/// The folders themselves aren't shown in problems: they can hold a username.
fn read_workspace_roots(
    section: &Section<'_>,
    items: &[Item<'_>],
    user_home: Option<&Path>,
    report: &mut Report,
) -> Vec<PathBuf> {
    let mut roots = Vec::with_capacity(items.len());
    for item in items {
        let number = item.number;
        let problem = if item.text.trim().is_empty() {
            format!("item {number} is empty.")
        } else {
            match expand_tilde(item.text, user_home) {
                Ok(path) if path.is_absolute() => {
                    roots.push(path);
                    continue;
                }
                Ok(_) => format!(
                    "item {number} must be a full path, or start with ~/ for your home folder."
                ),
                Err(_) => format!(
                    "item {number} starts with ~, but Wrybill can't find your home folder. Write the full path instead."
                ),
            }
        };
        section.error(item.line, "workspace_roots", problem, report);
    }
    roots
}

/// Checks each item of a list with `is_valid`, and lower-cases the good ones.
fn read_list(
    section: &Section<'_>,
    key: &str,
    items: &[Item<'_>],
    is_valid: fn(&str) -> bool,
    wanted: &str,
    report: &mut Report,
) -> Vec<String> {
    let mut values = Vec::with_capacity(items.len());
    for item in items {
        if is_valid(item.text) {
            values.push(item.text.to_ascii_lowercase());
        } else {
            let message = format!("item {} {wanted}", item.number);
            section.error(item.line, key, message, report);
        }
    }
    values
}

fn read_storage(mut section: Section<'_>, report: &mut Report) -> Storage {
    let default = Storage::default();
    let retention_days = section
        .whole("retention_days", 1, u64::from(u32::MAX), report)
        .and_then(|found| u32::try_from(found.value).ok())
        .unwrap_or(default.retention_days);
    let retention_max_gb = section
        .amount("retention_max_gb", Amount::AboveZero, report)
        .map_or(default.retention_max_gb, |found| found.value);
    section.finish(report);

    Storage {
        retention_days,
        retention_max_gb,
    }
}

fn read_model(mut section: Section<'_>, line: usize, report: &mut Report) -> ModelDraft {
    // The id comes first, so every later problem can say which model it means.
    let id = read_id(&mut section, report);
    require(
        &section,
        "id",
        line,
        "Every model needs an `id`: a name of your choice.",
        report,
    );

    let provider = section
        .choice("provider", &Provider::CHOICES, report)
        .map(|found| found.value);
    require(
        &section,
        "provider",
        line,
        "It says who serves the model: \"anthropic\", \"openai\", \"gemini\", \"openrouter\" or \"openai-compatible\".",
        report,
    );

    let model = section
        .filled_text("model", report)
        .map(|found| found.value.to_owned());
    require(
        &section,
        "model",
        line,
        "It's the provider's own name for the model.",
        report,
    );

    let roles = read_roles(&mut section, report);
    let hosted = provider.filter(|provider| provider.is_hosted());
    let api_key = read_key_ref(&mut section, hosted.map(Provider::as_str), report);
    let endpoint = read_url(&mut section, "endpoint", report);
    let locality = section
        .choice("locality", &Locality::CHOICES, report)
        .map(|found| found.value);
    if provider == Some(Provider::OpenaiCompatible) {
        require(
            &section,
            "endpoint",
            line,
            "An \"openai-compatible\" model needs the address of its server, such as http://127.0.0.1:11434/v1.",
            report,
        );
        require(
            &section,
            "locality",
            line,
            "Say where an \"openai-compatible\" model runs: \"local\" for this computer, \"lan\" for your own network, or \"cloud\".",
            report,
        );
    }

    let tools = section
        .choice("tools", &ToolCalling::CHOICES, report)
        .map(|found| found.value);
    let vision = section.flag("vision", report).map(|found| found.value);
    let context_window = section
        .whole("context_window", 1, i64::MAX.unsigned_abs(), report)
        .map(|found| found.value);
    let min_free_ram_gb = section
        .amount("min_free_ram_gb", Amount::AboveZero, report)
        .map(|found| found.value);
    let trusted_for_private = section
        .flag("trusted_for_private", report)
        .is_some_and(|found| found.value);
    let cost_tier = section
        .choice("cost_tier", &CostTier::CHOICES, report)
        .map(|found| found.value);
    let price_in_usd_per_mtok = section
        .amount("price_in_usd_per_mtok", Amount::ZeroOrMore, report)
        .map(|found| found.value);
    let price_out_usd_per_mtok = section
        .amount("price_out_usd_per_mtok", Amount::ZeroOrMore, report)
        .map(|found| found.value);
    let enabled = section
        .flag("enabled", report)
        .is_none_or(|found| found.value);
    let notes = section
        .text("notes", report)
        .map(|found| found.value.to_owned());
    section.finish(report);

    ModelDraft {
        id,
        provider,
        model,
        roles,
        api_key,
        endpoint,
        locality,
        tools,
        vision,
        context_window,
        min_free_ram_gb,
        trusted_for_private,
        cost_tier,
        price_in_usd_per_mtok,
        price_out_usd_per_mtok,
        enabled,
        notes,
    }
}

fn read_mcp_server(mut section: Section<'_>, line: usize, report: &mut Report) -> McpDraft {
    let id = read_id(&mut section, report);
    require(
        &section,
        "id",
        line,
        "Every MCP server needs an `id`: a name of your choice.",
        report,
    );

    let command = section
        .filled_text("command", report)
        .map(|found| found.value.to_owned());
    let args = section
        .texts("args", report)
        .map(|found| {
            let items = found.value.items.iter();
            items.map(|item| item.text.to_owned()).collect()
        })
        .unwrap_or_default();
    let url = read_url(&mut section, "url", report);
    match (section.has("command"), section.has("url")) {
        (true, true) => section.error_here(
            line,
            "has both `command` and `url`. Keep `command` for a server on this computer, or `url` for a remote one.",
            report,
        ),
        (false, false) => section.error_here(
            line,
            "needs either a `command`, for a server on this computer, or a `url`, for a remote one.",
            report,
        ),
        _ => {}
    }

    let trust = section
        .choice("trust", &McpTrust::CHOICES, report)
        .map(|found| found.value)
        .unwrap_or_default();
    let pass_env = match section.texts("pass_env", report) {
        Some(found) => {
            let mut names = Vec::with_capacity(found.value.items.len());
            for item in &found.value.items {
                if is_valid_variable_name(item.text) {
                    names.push(item.text.to_owned());
                } else {
                    let message = format!(
                        "item {} must be the name of an environment variable, such as GITHUB_TOKEN.",
                        item.number
                    );
                    section.error(item.line, "pass_env", message, report);
                }
            }
            names
        }
        None => Vec::new(),
    };
    section.finish(report);

    McpDraft {
        id,
        command,
        args,
        url,
        trust,
        pass_env,
    }
}

/// Reports a key that an entry can't do without.
fn require(section: &Section<'_>, key: &str, line: usize, why: &str, report: &mut Report) {
    if !section.has(key) {
        section.error(line, key, format!("is missing. {why}"), report);
    }
}

/// Reads an entry's `id`, and labels the entry with it for later problems.
fn read_id(section: &mut Section<'_>, report: &mut Report) -> Option<String> {
    let found = section.filled_text("id", report)?;
    if found
        .value
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        section.error(found.line, "id", "must be a name with no spaces.", report);
        return None;
    }
    section.set_label(found.value);
    Some(found.value.to_owned())
}

fn read_roles(section: &mut Section<'_>, report: &mut Report) -> Option<Vec<Role>> {
    let found = section.texts("roles", report)?;
    if found.value.written == 0 {
        section.error(
            found.line,
            "roles",
            "needs at least one role. To let Wrybill decide, remove the line.",
            report,
        );
        return None;
    }

    let mut roles = Vec::with_capacity(found.value.items.len());
    for item in &found.value.items {
        let number = item.number;
        match Role::CHOICES.iter().find(|(word, _)| *word == item.text) {
            Some((_, role)) => {
                if !roles.contains(role) {
                    roles.push(*role);
                }
            }
            None => {
                let message = if item.text == "vision" {
                    format!(
                        "item {number} is \"vision\", which isn't a role. To say a model accepts images, use `vision = true`."
                    )
                } else {
                    format!(
                        "item {number} must be {}, but it's {} here.",
                        quoted_list(Role::CHOICES.iter().map(|(word, _)| *word)),
                        shown_word(item.text)
                    )
                };
                section.error(item.line, "roles", message, report);
            }
        }
    }
    // Any item that was left out has been reported, so the config won't be built.
    Some(roles)
}

/// Reads an `api_key`. It has to be a reference, and its value is never
/// shown, because it may be a real key pasted in by mistake.
///
/// `provider` is the name the key would be saved under, when that's known.
fn read_key_ref(
    section: &mut Section<'_>,
    provider: Option<&str>,
    report: &mut Report,
) -> Option<KeyRef> {
    let found = section.text("api_key", report)?;
    match KeyRef::parse(found.value) {
        Ok(reference) => Some(reference),
        Err(error) => {
            let message = match error {
                KeyRefError::NotAReference => {
                    let fix = match provider {
                        Some(name) => format!(
                            "If it is, take it out of this file and run `wrybill keys set {name}`. This line can then go: Wrybill finds that key by itself."
                        ),
                        None => "If it is, take it out of this file, run `wrybill keys set <name>` with a name of your choice, and write `keychain:wrybill/<name>` here.".to_owned(),
                    };
                    format!(
                        "must point to a key, not hold one. Write `keychain:wrybill/<name>` or `env:VARIABLE_NAME`. The value isn't shown here in case it's a real key. {fix}"
                    )
                }
                KeyRefError::BadKeychainReference => {
                    "isn't a valid keychain reference. Write `keychain:wrybill/<name>`, where the name uses lowercase letters, digits, - and _.".to_owned()
                }
                KeyRefError::BadEnvReference => {
                    "isn't a valid environment reference. Write `env:` and then the variable's name, such as `env:ANTHROPIC_API_KEY`.".to_owned()
                }
            };
            section.error(found.line, "api_key", message, report);
            None
        }
    }
}

/// Reads a URL. Its value is never shown: a URL can carry a secret.
fn read_url(section: &mut Section<'_>, key: &'static str, report: &mut Report) -> Option<Url> {
    let found = section.text(key, report)?;
    let url = Url::parse(found.value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"));
    let Some(url) = url else {
        section.error(
            found.line,
            key,
            "must be a web address that starts with http:// or https://, such as http://127.0.0.1:8080/v1.",
            report,
        );
        return None;
    };
    if !url.username().is_empty() || url.password().is_some() {
        section.error(
            found.line,
            key,
            "can't hold a username or password. Keys stay out of this file: save one with `wrybill keys set`.",
            report,
        );
        return None;
    }
    Some(url)
}

/// A domain name on its own: at least two labels, no scheme, port or path.
fn is_domain_name(text: &str) -> bool {
    text.len() <= 253 && text.contains('.') && text.split('.').all(is_host_label)
}

/// A host, an IP address, or a pattern such as `*.test`.
fn is_host_pattern(text: &str) -> bool {
    if let Some(suffix) = text.strip_prefix("*.") {
        return suffix.split('.').all(is_host_label);
    }
    if let Some(address) = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return address.parse::<Ipv6Addr>().is_ok();
    }
    if text.parse::<Ipv4Addr>().is_ok() {
        return true;
    }
    // Dotted numbers that aren't an IP address are a typo, not a host name.
    let all_numbers = text
        .split('.')
        .all(|label| label.bytes().all(|byte| byte.is_ascii_digit()));
    !all_numbers && text.split('.').all(is_host_label)
}

/// One part of a host name: letters, digits and hyphens, not starting or
/// ending with a hyphen.
fn is_host_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

/// Builds the config from a file that had no problems.
pub(crate) fn build(draft: Draft) -> Config {
    Config {
        defaults: draft.defaults,
        limits: draft.limits,
        routing: draft.routing,
        search: draft.search.and_then(build_search),
        safety: draft.safety,
        storage: draft.storage,
        models: draft.models.into_iter().filter_map(build_model).collect(),
        mcp_servers: draft
            .mcp_servers
            .into_iter()
            .filter_map(build_mcp_server)
            .collect(),
    }
}

/// A `[search]` section with no provider leaves web search switched off.
fn build_search(draft: SearchDraft) -> Option<Search> {
    let provider = draft.provider?;
    let (api_key, endpoint) = if provider.takes_a_key() {
        // The key saved by `wrybill keys set <provider>`, unless the file names another.
        let api_key = draft
            .api_key
            .or_else(|| KeyRef::keychain(provider.as_str()));
        (api_key, None)
    } else {
        let endpoint = draft
            .endpoint
            .or_else(|| Url::parse(SEARXNG_DEFAULT_ENDPOINT).ok());
        (None, endpoint)
    };
    Some(Search {
        provider,
        api_key,
        endpoint,
    })
}

fn build_model(draft: ModelDraft) -> Option<Model> {
    let provider = draft.provider?;
    let hosted = provider.is_hosted();
    Some(Model {
        id: draft.id?,
        provider,
        model: draft.model?,
        roles: draft.roles,
        api_key: draft.api_key.or_else(|| {
            // The key saved by `wrybill keys set <provider>`.
            hosted
                .then(|| KeyRef::keychain(provider.as_str()))
                .flatten()
        }),
        endpoint: if hosted { None } else { draft.endpoint },
        locality: if hosted {
            Locality::Cloud
        } else {
            draft.locality?
        },
        tools: draft.tools.unwrap_or(if hosted {
            ToolCalling::Native
        } else {
            ToolCalling::Prompted
        }),
        vision: draft.vision,
        context_window: draft.context_window,
        min_free_ram_gb: draft.min_free_ram_gb,
        trusted_for_private: draft.trusted_for_private,
        cost_tier: draft.cost_tier,
        price_in_usd_per_mtok: draft.price_in_usd_per_mtok,
        price_out_usd_per_mtok: draft.price_out_usd_per_mtok,
        enabled: draft.enabled,
        notes: draft.notes,
    })
}

fn build_mcp_server(draft: McpDraft) -> Option<McpServer> {
    let transport = match (draft.command, draft.url) {
        (Some(command), None) => McpTransport::Local {
            command,
            args: draft.args,
        },
        (None, Some(url)) => McpTransport::Remote { url },
        _ => return None,
    };
    Some(McpServer {
        id: draft.id?,
        transport,
        trust: draft.trust,
        pass_env: draft.pass_env,
    })
}

#[cfg(test)]
mod tests {
    use super::{is_domain_name, is_host_pattern};

    #[test]
    fn domain_names_stand_on_their_own() {
        for name in [
            "nodejs.org",
            "sh.rustup.rs",
            "www.python.org",
            "Example.COM",
        ] {
            assert!(is_domain_name(name), "{name}");
        }
        for name in [
            "",
            "localhost",
            "https://nodejs.org",
            "nodejs.org/download",
            "nodejs.org:443",
            "nodejs..org",
            "-bad.org",
            "under_score.org",
            "nodejs.org.",
        ] {
            assert!(!is_domain_name(name), "{name}");
        }
    }

    #[test]
    fn host_patterns_cover_names_addresses_and_wildcards() {
        for pattern in [
            "localhost",
            "127.0.0.1",
            "[::1]",
            "*.localhost",
            "*.test",
            "myapp.test",
            "*.dev.localhost",
        ] {
            assert!(is_host_pattern(pattern), "{pattern}");
        }
        for pattern in [
            "",
            "*",
            "*.",
            "*test",
            "::1",
            "[not-an-address]",
            "999.1.1.1",
            "http://localhost",
            "localhost:3000",
            "local host",
        ] {
            assert!(!is_host_pattern(pattern), "{pattern}");
        }
    }
}
