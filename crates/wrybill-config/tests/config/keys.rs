//! One key at a time: what each key in spec 13.3 accepts, what it defaults
//! to, and what its owner is told when it's wrong.

use std::path::Path;

use wrybill_config::{
    Autonomy, Config, CostTier, Defaults, KeyRef, Limits, LocalToCloud, Locality, McpServer,
    McpTransport, McpTrust, Model, Provider, Role, Routing, Safety, Search, SearchProvider,
    Storage, ToolCalling, Url, parse,
};

use crate::support::{
    home, hosted_model, local_model, mcp_server, problem_tests, problems, section, valid,
};

const NOT_A_URL: &str =
    "must be a web address that starts with http:// or https://, such as http://127.0.0.1:8080/v1.";
const NO_LOGIN_IN_URL: &str = "can't hold a username or password. Keys stay out of this file: save one with `wrybill keys set`.";
const BAD_KEYCHAIN_REFERENCE: &str = "isn't a valid keychain reference. Write `keychain:wrybill/<name>`, where the name uses lowercase letters, digits, - and _.";
const BAD_ENV_REFERENCE: &str = "isn't a valid environment reference. Write `env:` and then the variable's name, such as `env:ANTHROPIC_API_KEY`.";

/// Something shaped like a key. No test may ever find it in a problem.
const MARKER: &str = "sk-test-MARKER-0123456789abcdef";

fn key(name: &str) -> KeyRef {
    KeyRef::keychain(name).expect("a valid key name")
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("a valid URL")
}

// The top of the file.

problem_tests! {
    an_empty_file_needs_a_version:
        "" => ["version: is missing. Put `version = 1` at the top of the file."];
    version_has_to_be_1:
        "version = 2\n" => ["line 1: version: must be 1, the only config version this Wrybill reads."];
    version_has_to_be_a_number:
        "version = \"1\"\n" => ["line 1: version: must be 1, the only config version this Wrybill reads."];
    an_unknown_key_at_the_top_is_named:
        "version = 1\ncolour = \"blue\"\n" => ["line 2: colour: isn't a key Wrybill knows."];
    an_unknown_section_is_named_with_a_suggestion:
        "version = 1\n[limts]\nmax_steps_per_task = 5\n" => ["line 2: [limts]: isn't a section Wrybill knows. Did you mean [limits]?"];
    a_section_cannot_be_a_plain_value:
        "version = 1\ndefaults = 3\n" => ["line 2: defaults: must be a section, written as [defaults] on a line of its own, but it's a whole number here."];
    models_have_to_be_a_list_of_entries:
        "version = 1\n[models]\nid = \"a\"\n" => ["line 2: models: must be a list of entries. Start each one with [[models]], with two pairs of square brackets."];
    mcp_servers_have_to_be_a_list_of_entries:
        "version = 1\nmcp_servers = \"files\"\n" => ["line 2: mcp_servers: must be a list of entries. Start each one with [[mcp_servers]], with two pairs of square brackets."];
}

#[test]
fn a_file_that_is_not_valid_toml_says_where_it_stops() {
    let found = problems("version = 1\n[defaults]\nplanner = \"abc\n");

    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .starts_with("line 3: Wrybill can't read the file from here on. It isn't valid TOML: "),
        "{found:?}"
    );
}

#[test]
fn a_file_with_only_a_version_gives_the_built_in_defaults() {
    assert_eq!(valid("version = 1\n"), Config::defaults(Some(&home())));
}

#[test]
fn the_built_in_defaults_are_the_ones_in_spec_13_3() {
    let config = Config::defaults(Some(&home()));

    assert_eq!(
        config.defaults,
        Defaults {
            planner: None,
            worker: None,
            summariser: None,
            autonomy: Autonomy::Ask,
        }
    );
    assert_eq!(
        config.limits,
        Limits {
            max_parallel_cloud_helpers: 3,
            max_parallel_calls_per_local_server: 4,
            max_helper_depth: 1,
            max_steps_per_task: 60,
            max_minutes_per_task: 30,
            max_tokens_per_task: 2_000_000,
            max_spend_usd_per_task: 2.0,
        }
    );
    assert_eq!(
        config.routing,
        Routing {
            allow_cloud: true,
            local_to_cloud: LocalToCloud::Ask,
        }
    );
    assert_eq!(config.search, None);
    assert_eq!(
        config.safety,
        Safety {
            workspace_roots: vec![home().join("Wrybill").join("workspaces")],
            trusted_install_domains: vec![],
            dev_hosts: ["localhost", "127.0.0.1", "[::1]", "*.localhost", "*.test"]
                .map(str::to_owned)
                .to_vec(),
            warn_on_unsupported_os: true,
        }
    );
    assert_eq!(
        config.storage,
        Storage {
            retention_days: 30,
            retention_max_gb: 2.0,
        }
    );
    assert!(config.models.is_empty());
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn every_key_takes_a_valid_value() {
    let config = valid(
        r#"
version = 1

[defaults]
planner = "big"
worker = "small"
summariser = "small"
autonomy = "plan"

[limits]
max_parallel_cloud_helpers = 5
max_parallel_calls_per_local_server = 2
max_helper_depth = 1
max_steps_per_task = 100
max_minutes_per_task = 45
max_tokens_per_task = 3_000_000
max_spend_usd_per_task = 4.5

[routing]
allow_cloud = true
local_to_cloud = "never"

[search]
provider = "searxng"
endpoint = "http://search.lan:8080"

[safety]
workspace_roots = ["~/Work", "~/Projects/wrybill"]
trusted_install_domains = ["Nodejs.org"]
dev_hosts = ["localhost", "*.internal"]
warn_on_unsupported_os = false

[storage]
retention_days = 7
retention_max_gb = 0.5

[[models]]
id = "big"
provider = "openrouter"
model = "vendor/model-name"
roles = ["planner"]
api_key = "env:OPENROUTER_API_KEY"
tools = "prompted"
vision = true
context_window = 200000
cost_tier = "high"
price_in_usd_per_mtok = 3
price_out_usd_per_mtok = 15.5
enabled = true
notes = "The strong one."

[[models]]
id = "small"
provider = "openai-compatible"
endpoint = "http://192.168.1.20:11434/v1"
model = "qwen3:4b"
roles = ["worker", "summariser"]
api_key = "keychain:wrybill/lan-box"
locality = "lan"
tools = "none"
vision = false
context_window = 8192
trusted_for_private = true

[[models]]
id = "spare"
provider = "openai-compatible"
endpoint = "http://127.0.0.1:8080/v1"
model = "tiny"
locality = "local"
min_free_ram_gb = 3
cost_tier = "low"
enabled = false

[[mcp_servers]]
id = "files"
command = "files-server"
args = ["--root", "."]
trust = "allow"
pass_env = ["FILES_TOKEN"]

[[mcp_servers]]
id = "remote"
url = "https://mcp.example.com/sse"
"#,
    );

    let expected = Config {
        defaults: Defaults {
            planner: Some("big".to_owned()),
            worker: Some("small".to_owned()),
            summariser: Some("small".to_owned()),
            autonomy: Autonomy::Plan,
        },
        limits: Limits {
            max_parallel_cloud_helpers: 5,
            max_parallel_calls_per_local_server: 2,
            max_helper_depth: 1,
            max_steps_per_task: 100,
            max_minutes_per_task: 45,
            max_tokens_per_task: 3_000_000,
            max_spend_usd_per_task: 4.5,
        },
        routing: Routing {
            allow_cloud: true,
            local_to_cloud: LocalToCloud::Never,
        },
        search: Some(Search {
            provider: SearchProvider::Searxng,
            api_key: None,
            endpoint: Some(url("http://search.lan:8080")),
        }),
        safety: Safety {
            workspace_roots: vec![home().join("Work"), home().join("Projects").join("wrybill")],
            trusted_install_domains: vec!["nodejs.org".to_owned()],
            dev_hosts: vec!["localhost".to_owned(), "*.internal".to_owned()],
            warn_on_unsupported_os: false,
        },
        storage: Storage {
            retention_days: 7,
            retention_max_gb: 0.5,
        },
        models: vec![
            Model {
                id: "big".to_owned(),
                provider: Provider::Openrouter,
                model: "vendor/model-name".to_owned(),
                roles: Some(vec![Role::Planner]),
                api_key: Some(KeyRef::Env {
                    variable: "OPENROUTER_API_KEY".to_owned(),
                }),
                endpoint: None,
                locality: Locality::Cloud,
                tools: ToolCalling::Prompted,
                vision: Some(true),
                context_window: Some(200_000),
                min_free_ram_gb: None,
                trusted_for_private: false,
                cost_tier: Some(CostTier::High),
                price_in_usd_per_mtok: Some(3.0),
                price_out_usd_per_mtok: Some(15.5),
                enabled: true,
                notes: Some("The strong one.".to_owned()),
            },
            Model {
                id: "small".to_owned(),
                provider: Provider::OpenaiCompatible,
                model: "qwen3:4b".to_owned(),
                roles: Some(vec![Role::Worker, Role::Summariser]),
                api_key: Some(key("lan-box")),
                endpoint: Some(url("http://192.168.1.20:11434/v1")),
                locality: Locality::Lan,
                tools: ToolCalling::None,
                vision: Some(false),
                context_window: Some(8192),
                min_free_ram_gb: None,
                trusted_for_private: true,
                cost_tier: None,
                price_in_usd_per_mtok: None,
                price_out_usd_per_mtok: None,
                enabled: true,
                notes: None,
            },
            Model {
                id: "spare".to_owned(),
                provider: Provider::OpenaiCompatible,
                model: "tiny".to_owned(),
                roles: None,
                api_key: None,
                endpoint: Some(url("http://127.0.0.1:8080/v1")),
                locality: Locality::Local,
                tools: ToolCalling::Prompted,
                vision: None,
                context_window: None,
                min_free_ram_gb: Some(3.0),
                trusted_for_private: false,
                cost_tier: Some(CostTier::Low),
                price_in_usd_per_mtok: None,
                price_out_usd_per_mtok: None,
                enabled: false,
                notes: None,
            },
        ],
        mcp_servers: vec![
            McpServer {
                id: "files".to_owned(),
                transport: McpTransport::Local {
                    command: "files-server".to_owned(),
                    args: vec!["--root".to_owned(), ".".to_owned()],
                },
                trust: McpTrust::Allow,
                pass_env: vec!["FILES_TOKEN".to_owned()],
            },
            McpServer {
                id: "remote".to_owned(),
                transport: McpTransport::Remote {
                    url: url("https://mcp.example.com/sse"),
                },
                trust: McpTrust::Ask,
                pass_env: vec![],
            },
        ],
    };
    assert_eq!(config, expected);
}

// [defaults]

problem_tests! {
    planner_has_to_be_text:
        section("defaults", "planner = 3") => ["line 3: [defaults] planner: must be text in quotes, but it's a whole number here."];
    worker_cannot_be_empty:
        section("defaults", "worker = \"\"") => ["line 3: [defaults] worker: can't be empty."];
    summariser_has_to_be_text:
        section("defaults", "summariser = true") => ["line 3: [defaults] summariser: must be text in quotes, but it's true or false here."];
    autonomy_is_one_of_three_words:
        section("defaults", "autonomy = \"atuo\"") => ["line 3: [defaults] autonomy: must be \"plan\", \"ask\" or \"auto\", but it's \"atuo\" here."];
    autonomy_has_to_be_text:
        section("defaults", "autonomy = 1") => ["line 3: [defaults] autonomy: must be \"plan\", \"ask\" or \"auto\", but it's a whole number here."];
    a_mistyped_key_gets_a_suggestion:
        section("defaults", "plannner = \"big\"") => ["line 3: [defaults] plannner: isn't a key Wrybill knows. Did you mean `planner`?"];
    a_key_from_another_section_is_unknown_here:
        section("defaults", "retention_days = 3") => ["line 3: [defaults] retention_days: isn't a key Wrybill knows."];
}

// [limits]

problem_tests! {
    max_parallel_cloud_helpers_is_1_or_more:
        section("limits", "max_parallel_cloud_helpers = 0") => ["line 3: [limits] max_parallel_cloud_helpers: must be 1 or more, but it's 0 here."];
    max_parallel_cloud_helpers_has_a_ceiling:
        section("limits", "max_parallel_cloud_helpers = 4294967296") => ["line 3: [limits] max_parallel_cloud_helpers: is too large. The most it can be is 4294967295."];
    max_parallel_calls_per_local_server_is_a_whole_number:
        section("limits", "max_parallel_calls_per_local_server = 1.5") => ["line 3: [limits] max_parallel_calls_per_local_server: must be a whole number, 1 or more, but it's a decimal number here."];
    max_helper_depth_cannot_be_higher_than_1:
        section("limits", "max_helper_depth = 2") => ["line 3: [limits] max_helper_depth: must be 1 in this version of Wrybill."];
    max_helper_depth_cannot_be_lower_than_1:
        section("limits", "max_helper_depth = 0") => ["line 3: [limits] max_helper_depth: must be 1 in this version of Wrybill."];
    max_steps_per_task_is_not_text:
        section("limits", "max_steps_per_task = \"60\"") => ["line 3: [limits] max_steps_per_task: must be a whole number, 1 or more, but it's text here."];
    max_minutes_per_task_is_not_negative:
        section("limits", "max_minutes_per_task = -5") => ["line 3: [limits] max_minutes_per_task: must be 1 or more, but it's -5 here."];
    max_tokens_per_task_is_1_or_more:
        section("limits", "max_tokens_per_task = 0") => ["line 3: [limits] max_tokens_per_task: must be 1 or more, but it's 0 here."];
    max_spend_usd_per_task_is_above_0:
        section("limits", "max_spend_usd_per_task = 0") => ["line 3: [limits] max_spend_usd_per_task: must be a number above 0, but it's 0 here."];
    max_spend_usd_per_task_is_not_negative:
        section("limits", "max_spend_usd_per_task = -1.5") => ["line 3: [limits] max_spend_usd_per_task: must be a number above 0, but it's -1.5 here."];
    max_spend_usd_per_task_is_not_text:
        section("limits", "max_spend_usd_per_task = \"2\"") => ["line 3: [limits] max_spend_usd_per_task: must be a number above 0, but it's text here."];
    max_spend_usd_per_task_is_an_ordinary_number:
        section("limits", "max_spend_usd_per_task = inf") => ["line 3: [limits] max_spend_usd_per_task: must be a number above 0."];
}

#[test]
fn a_decimal_key_takes_a_whole_number_too() {
    let config = valid(&section("limits", "max_spend_usd_per_task = 2"));

    assert_eq!(config.limits.max_spend_usd_per_task, 2.0);
}

#[test]
fn a_big_number_can_be_written_with_underscores() {
    let config = valid(&section("limits", "max_tokens_per_task = 5_000_000_000"));

    assert_eq!(config.limits.max_tokens_per_task, 5_000_000_000);
}

// [routing]

problem_tests! {
    allow_cloud_has_no_quotes:
        section("routing", "allow_cloud = \"false\"") => ["line 3: [routing] allow_cloud: must be true or false without quotes."];
    allow_cloud_is_true_or_false:
        section("routing", "allow_cloud = 1") => ["line 3: [routing] allow_cloud: must be true or false, but it's a whole number here."];
    local_to_cloud_is_one_of_two_words:
        section("routing", "local_to_cloud = \"always\"") => ["line 3: [routing] local_to_cloud: must be \"ask\" or \"never\", but it's \"always\" here."];
}

// [search]

problem_tests! {
    search_provider_is_one_of_four:
        section("search", "provider = \"google\"") => ["line 3: [search] provider: must be \"brave\", \"exa\", \"tavily\" or \"searxng\", but it's \"google\" here."];
    search_api_key_cannot_be_the_key_itself:
        section("search", &format!("provider = \"brave\"\napi_key = \"{MARKER}\"")) => ["line 4: [search] api_key: must point to a key, not hold one. Write `keychain:wrybill/<name>` or `env:VARIABLE_NAME`. The value isn't shown here in case it's a real key. If it is, take it out of this file and run `wrybill keys set brave`. This line can then go: Wrybill finds that key by itself."];
    search_api_key_needs_a_whole_keychain_reference:
        section("search", "provider = \"brave\"\napi_key = \"keychain:brave\"") => [format!("line 4: [search] api_key: {BAD_KEYCHAIN_REFERENCE}")];
    search_api_key_needs_a_variable_name:
        section("search", "provider = \"brave\"\napi_key = \"env:\"") => [format!("line 4: [search] api_key: {BAD_ENV_REFERENCE}")];
    search_endpoint_has_to_be_a_web_address:
        section("search", "provider = \"searxng\"\nendpoint = \"localhost:8888\"") => [format!("line 4: [search] endpoint: {NOT_A_URL}")];
    search_endpoint_cannot_hold_a_login:
        section("search", "provider = \"searxng\"\nendpoint = \"http://sam:pw@127.0.0.1:8888\"") => [format!("line 4: [search] endpoint: {NO_LOGIN_IN_URL}")];
}

#[test]
fn a_search_provider_finds_its_key_by_name() {
    let config = valid(&section("search", "provider = \"tavily\""));

    assert_eq!(
        config.search,
        Some(Search {
            provider: SearchProvider::Tavily,
            api_key: Some(key("tavily")),
            endpoint: None,
        })
    );
}

#[test]
fn a_search_key_can_be_named_or_come_from_the_environment() {
    let named = valid(&section(
        "search",
        "provider = \"brave\"\napi_key = \"keychain:wrybill/brave-work\"",
    ));
    let from_env = valid(&section(
        "search",
        "provider = \"exa\"\napi_key = \"env:EXA_API_KEY\"",
    ));

    assert_eq!(
        named.search.and_then(|search| search.api_key),
        Some(key("brave-work"))
    );
    assert_eq!(
        from_env.search.and_then(|search| search.api_key),
        Some(KeyRef::Env {
            variable: "EXA_API_KEY".to_owned()
        })
    );
}

#[test]
fn searxng_needs_no_key_and_defaults_to_its_usual_local_address() {
    let config = valid(&section("search", "provider = \"searxng\""));

    assert_eq!(
        config.search,
        Some(Search {
            provider: SearchProvider::Searxng,
            api_key: None,
            endpoint: Some(url("http://127.0.0.1:8888")),
        })
    );
}

#[test]
fn an_empty_search_section_leaves_search_off() {
    assert_eq!(valid("version = 1\n[search]\n").search, None);
}

// [safety]

problem_tests! {
    workspace_roots_is_a_list:
        section("safety", "workspace_roots = \"~/Projects\"") => ["line 3: [safety] workspace_roots: must be a list in square brackets, such as [\"a\", \"b\"], but it's text here."];
    workspace_roots_are_full_paths:
        section("safety", "workspace_roots = [\"Projects\"]") => ["line 3: [safety] workspace_roots: item 1 must be a full path, or start with ~/ for your home folder."];
    workspace_roots_cannot_be_empty_text:
        section("safety", "workspace_roots = [\"~/ok\", \"\"]") => ["line 3: [safety] workspace_roots: item 2 is empty."];
    every_bad_workspace_root_is_reported:
        section("safety", "workspace_roots = [\n  \"Projects\",\n  \"~/ok\",\n  3,\n]") => [
            "line 4: [safety] workspace_roots: item 1 must be a full path, or start with ~/ for your home folder.",
            "line 6: [safety] workspace_roots: item 3 must be text in quotes, but it's a whole number here.",
        ];
    trusted_install_domains_are_bare_domain_names:
        section("safety", "trusted_install_domains = [\"nodejs.org\", \"https://www.python.org\"]") => ["line 3: [safety] trusted_install_domains: item 2 must be a domain name on its own, such as nodejs.org, with no https:// and no path."];
    dev_hosts_cannot_be_a_catch_all:
        section("safety", "dev_hosts = [\"localhost\", \"*\"]") => ["line 3: [safety] dev_hosts: item 2 must be a host name, an IP address, or a pattern such as *.test."];
    warn_on_unsupported_os_is_true_or_false:
        section("safety", "warn_on_unsupported_os = \"yes\"") => ["line 3: [safety] warn_on_unsupported_os: must be true or false, but it's text here."];
}

#[test]
fn a_tilde_needs_a_known_home_folder() {
    let text = section("safety", "workspace_roots = [\"~/Projects\"]");

    let invalid = parse(&text, None).expect_err("a config with a problem");

    assert_eq!(
        invalid.errors[0].to_string(),
        "line 3: [safety] workspace_roots: item 1 starts with ~, but Wrybill can't find your home folder. Write the full path instead."
    );
}

#[test]
fn a_workspace_root_can_be_a_full_path() {
    let full_path = home().join("Projects");
    // Written as a TOML literal string, so Windows backslashes need no escaping.
    let text = section(
        "safety",
        &format!("workspace_roots = ['{}']", full_path.display()),
    );

    assert_eq!(valid(&text).safety.workspace_roots, vec![full_path]);
}

#[test]
fn an_empty_list_of_workspace_roots_is_allowed() {
    let config = valid(&section("safety", "workspace_roots = []"));

    assert_eq!(config.safety.workspace_roots, Vec::<&Path>::new());
}

// [storage]

problem_tests! {
    retention_days_is_1_or_more:
        section("storage", "retention_days = 0") => ["line 3: [storage] retention_days: must be 1 or more, but it's 0 here."];
    retention_days_is_a_whole_number:
        section("storage", "retention_days = 1.5") => ["line 3: [storage] retention_days: must be a whole number, 1 or more, but it's a decimal number here."];
    retention_max_gb_is_above_0:
        section("storage", "retention_max_gb = 0") => ["line 3: [storage] retention_max_gb: must be a number above 0, but it's 0 here."];
}

#[test]
fn retention_max_gb_can_be_a_fraction() {
    let config = valid(&section("storage", "retention_max_gb = 0.5"));

    assert_eq!(config.storage.retention_max_gb, 0.5);
}

// [[models]]

problem_tests! {
    a_model_needs_an_id_a_provider_and_a_model:
        "version = 1\n\n[[models]]\nnotes = \"nothing else\"\n" => [
            "line 3: [[models]] entry 1, id: is missing. Every model needs an `id`: a name of your choice.",
            "line 3: [[models]] entry 1, provider: is missing. It says who serves the model: \"anthropic\", \"openai\", \"gemini\", \"openrouter\" or \"openai-compatible\".",
            "line 3: [[models]] entry 1, model: is missing. It's the provider's own name for the model.",
        ];
    a_model_id_has_no_spaces:
        "version = 1\n[[models]]\nid = \"my model\"\nprovider = \"anthropic\"\nmodel = \"x\"\n" => ["line 3: [[models]] entry 1, id: must be a name with no spaces."];
    a_model_id_cannot_be_empty:
        "version = 1\n[[models]]\nid = \"\"\nprovider = \"anthropic\"\nmodel = \"x\"\n" => ["line 3: [[models]] entry 1, id: can't be empty."];
    provider_is_one_of_five:
        "version = 1\n[[models]]\nid = \"m\"\nprovider = \"claude\"\nmodel = \"x\"\n" => ["line 4: [[models]] entry 1 (\"m\"), provider: must be \"anthropic\", \"openai\", \"gemini\", \"openrouter\" or \"openai-compatible\", but it's \"claude\" here."];
    a_model_name_cannot_be_empty:
        "version = 1\n[[models]]\nid = \"m\"\nprovider = \"anthropic\"\nmodel = \"\"\n" => ["line 5: [[models]] entry 1 (\"m\"), model: can't be empty."];
    roles_is_a_list:
        hosted_model("roles = \"planner\"") => ["line 6: [[models]] entry 1 (\"m\"), roles: must be a list in square brackets, such as [\"a\", \"b\"], but it's text here."];
    roles_needs_at_least_one_role:
        hosted_model("roles = []") => ["line 6: [[models]] entry 1 (\"m\"), roles: needs at least one role. To let Wrybill decide, remove the line."];
    roles_are_planner_worker_or_summariser:
        hosted_model("roles = [\"planner\", \"plan\"]") => ["line 6: [[models]] entry 1 (\"m\"), roles: item 2 must be \"planner\", \"worker\" or \"summariser\", but it's \"plan\" here."];
    vision_is_no_longer_a_role:
        hosted_model("roles = [\"worker\", \"vision\"]") => ["line 6: [[models]] entry 1 (\"m\"), roles: item 2 is \"vision\", which isn't a role. To say a model accepts images, use `vision = true`."];
    a_model_api_key_cannot_be_the_key_itself:
        hosted_model(&format!("api_key = \"{MARKER}\"")) => ["line 6: [[models]] entry 1 (\"m\"), api_key: must point to a key, not hold one. Write `keychain:wrybill/<name>` or `env:VARIABLE_NAME`. The value isn't shown here in case it's a real key. If it is, take it out of this file and run `wrybill keys set anthropic`. This line can then go: Wrybill finds that key by itself."];
    a_local_model_api_key_cannot_be_the_key_itself:
        local_model(&format!("api_key = \"{MARKER}\"")) => ["line 8: [[models]] entry 1 (\"m\"), api_key: must point to a key, not hold one. Write `keychain:wrybill/<name>` or `env:VARIABLE_NAME`. The value isn't shown here in case it's a real key. If it is, take it out of this file, run `wrybill keys set <name>` with a name of your choice, and write `keychain:wrybill/<name>` here."];
    a_model_api_key_has_to_use_the_wrybill_keychain_service:
        hosted_model("api_key = \"keychain:other/anthropic\"") => [format!("line 6: [[models]] entry 1 (\"m\"), api_key: {BAD_KEYCHAIN_REFERENCE}")];
    a_model_api_key_env_reference_needs_a_real_variable_name:
        hosted_model("api_key = \"env:MY-KEY\"") => [format!("line 6: [[models]] entry 1 (\"m\"), api_key: {BAD_ENV_REFERENCE}")];
    a_model_api_key_has_to_be_text:
        hosted_model("api_key = 12345") => ["line 6: [[models]] entry 1 (\"m\"), api_key: must be text in quotes, but it's a whole number here."];
    a_model_endpoint_has_to_be_a_web_address:
        local_model("").replace("http://127.0.0.1:11434/v1", "127.0.0.1:11434") => [format!("line 6: [[models]] entry 1 (\"m\"), endpoint: {NOT_A_URL}")];
    a_model_endpoint_cannot_use_another_scheme:
        local_model("").replace("http://127.0.0.1:11434/v1", "ftp://127.0.0.1/models") => [format!("line 6: [[models]] entry 1 (\"m\"), endpoint: {NOT_A_URL}")];
    a_model_endpoint_cannot_hold_a_login:
        local_model("").replace("http://127.0.0.1:11434/v1", &format!("http://sam:{MARKER}@127.0.0.1:11434/v1")) => [format!("line 6: [[models]] entry 1 (\"m\"), endpoint: {NO_LOGIN_IN_URL}")];
    an_openai_compatible_model_needs_an_endpoint_and_a_locality:
        "version = 1\n[[models]]\nid = \"m\"\nprovider = \"openai-compatible\"\nmodel = \"x\"\n" => [
            "line 2: [[models]] entry 1 (\"m\"), endpoint: is missing. An \"openai-compatible\" model needs the address of its server, such as http://127.0.0.1:11434/v1.",
            "line 2: [[models]] entry 1 (\"m\"), locality: is missing. Say where an \"openai-compatible\" model runs: \"local\" for this computer, \"lan\" for your own network, or \"cloud\".",
        ];
    locality_is_one_of_three:
        local_model("").replace("\"local\"", "\"remote\"") => ["line 7: [[models]] entry 1 (\"m\"), locality: must be \"cloud\", \"lan\" or \"local\", but it's \"remote\" here."];
    tools_is_one_of_three:
        hosted_model("tools = \"yes\"") => ["line 6: [[models]] entry 1 (\"m\"), tools: must be \"native\", \"prompted\" or \"none\", but it's \"yes\" here."];
    vision_is_true_or_false:
        hosted_model("vision = \"true\"") => ["line 6: [[models]] entry 1 (\"m\"), vision: must be true or false without quotes."];
    context_window_is_1_or_more:
        hosted_model("context_window = 0") => ["line 6: [[models]] entry 1 (\"m\"), context_window: must be 1 or more, but it's 0 here."];
    min_free_ram_gb_is_above_0:
        local_model("min_free_ram_gb = 0") => ["line 8: [[models]] entry 1 (\"m\"), min_free_ram_gb: must be a number above 0, but it's 0 here."];
    min_free_ram_gb_is_a_number:
        local_model("min_free_ram_gb = \"6\"") => ["line 8: [[models]] entry 1 (\"m\"), min_free_ram_gb: must be a number above 0, but it's text here."];
    trusted_for_private_is_true_or_false:
        local_model("trusted_for_private = \"yes\"") => ["line 8: [[models]] entry 1 (\"m\"), trusted_for_private: must be true or false, but it's text here."];
    cost_tier_is_one_of_three:
        hosted_model("cost_tier = \"cheap\"") => ["line 6: [[models]] entry 1 (\"m\"), cost_tier: must be \"low\", \"medium\" or \"high\", but it's \"cheap\" here."];
    price_in_is_not_negative:
        hosted_model("price_in_usd_per_mtok = -1") => ["line 6: [[models]] entry 1 (\"m\"), price_in_usd_per_mtok: must be a number, 0 or more, but it's -1 here."];
    price_out_is_a_number:
        hosted_model("price_out_usd_per_mtok = \"3\"") => ["line 6: [[models]] entry 1 (\"m\"), price_out_usd_per_mtok: must be a number, 0 or more, but it's text here."];
    enabled_is_true_or_false:
        hosted_model("enabled = 0") => ["line 6: [[models]] entry 1 (\"m\"), enabled: must be true or false, but it's a whole number here."];
    notes_is_text:
        hosted_model("notes = 5") => ["line 6: [[models]] entry 1 (\"m\"), notes: must be text in quotes, but it's a whole number here."];
    a_mistyped_model_key_gets_a_suggestion_and_its_value_stays_hidden:
        hosted_model(&format!("api_kye = \"{MARKER}\"")) => ["line 6: [[models]] entry 1 (\"m\"), api_kye: isn't a key Wrybill knows. Did you mean `api_key`?"];
    each_model_is_counted_and_named:
        "version = 1\n[[models]]\nid = \"first\"\nprovider = \"anthropic\"\nmodel = \"x\"\n\n[[models]]\nid = \"second\"\nprovider = \"anthropic\"\nmodel = \"x\"\ntools = 1\n" => ["line 11: [[models]] entry 2 (\"second\"), tools: must be \"native\", \"prompted\" or \"none\", but it's a whole number here."];
}

#[test]
fn a_hosted_model_needs_only_three_lines() {
    let config = valid(&hosted_model(""));

    assert_eq!(
        config.models,
        vec![Model {
            id: "m".to_owned(),
            provider: Provider::Anthropic,
            model: "x".to_owned(),
            roles: None,
            // The key saved by `wrybill keys set anthropic`.
            api_key: Some(key("anthropic")),
            endpoint: None,
            locality: Locality::Cloud,
            tools: ToolCalling::Native,
            vision: None,
            context_window: None,
            min_free_ram_gb: None,
            trusted_for_private: false,
            cost_tier: None,
            price_in_usd_per_mtok: None,
            price_out_usd_per_mtok: None,
            enabled: true,
            notes: None,
        }]
    );
}

#[test]
fn each_hosted_provider_finds_its_key_by_name() {
    for provider in ["anthropic", "openai", "gemini", "openrouter"] {
        let config = valid(&hosted_model("").replace("anthropic", provider));

        assert_eq!(config.models[0].api_key, Some(key(provider)), "{provider}");
        assert_eq!(config.models[0].provider.as_str(), provider);
    }
}

#[test]
fn a_local_model_has_no_key_and_prompted_tools_unless_it_says_otherwise() {
    let config = valid(&local_model(""));

    assert_eq!(config.models[0].api_key, None);
    assert_eq!(config.models[0].tools, ToolCalling::Prompted);
    assert_eq!(config.models[0].locality, Locality::Local);
}

#[test]
fn a_role_listed_twice_counts_once() {
    let config = valid(&hosted_model(
        "roles = [\"worker\", \"planner\", \"worker\"]",
    ));

    assert_eq!(
        config.models[0].roles,
        Some(vec![Role::Worker, Role::Planner])
    );
}

#[test]
fn a_long_or_odd_model_id_is_never_echoed() {
    let text = format!(
        "version = 1\n[[models]]\nid = \"{MARKER}\"\nprovider = \"anthropic\"\nmodel = \"x\"\ntools = 1\n"
    );

    assert_eq!(
        problems(&text),
        [
            "line 6: [[models]] entry 1, tools: must be \"native\", \"prompted\" or \"none\", but it's a whole number here."
        ]
    );
}

// [[mcp_servers]]

problem_tests! {
    an_mcp_server_needs_an_id:
        "version = 1\n[[mcp_servers]]\ncommand = \"server\"\n" => ["line 2: [[mcp_servers]] entry 1, id: is missing. Every MCP server needs an `id`: a name of your choice."];
    an_mcp_server_needs_a_command_or_a_url:
        "version = 1\n[[mcp_servers]]\nid = \"s\"\n" => ["line 2: [[mcp_servers]] entry 1 (\"s\"): needs either a `command`, for a server on this computer, or a `url`, for a remote one."];
    an_mcp_server_cannot_have_both_a_command_and_a_url:
        mcp_server("url = \"https://mcp.example.com\"") => ["line 2: [[mcp_servers]] entry 1 (\"s\"): has both `command` and `url`. Keep `command` for a server on this computer, or `url` for a remote one."];
    an_mcp_command_cannot_be_empty:
        "version = 1\n[[mcp_servers]]\nid = \"s\"\ncommand = \"\"\n" => ["line 4: [[mcp_servers]] entry 1 (\"s\"), command: can't be empty."];
    mcp_args_is_a_list:
        mcp_server("args = \"--root .\"") => ["line 5: [[mcp_servers]] entry 1 (\"s\"), args: must be a list in square brackets, such as [\"a\", \"b\"], but it's text here."];
    mcp_args_are_text:
        mcp_server("args = [\"--port\", 8080]") => ["line 5: [[mcp_servers]] entry 1 (\"s\"), args: item 2 must be text in quotes, but it's a whole number here."];
    an_mcp_url_has_to_be_a_web_address:
        "version = 1\n[[mcp_servers]]\nid = \"s\"\nurl = \"mcp.example.com\"\n" => [format!("line 4: [[mcp_servers]] entry 1 (\"s\"), url: {NOT_A_URL}")];
    an_mcp_url_cannot_hold_a_login:
        "version = 1\n[[mcp_servers]]\nid = \"s\"\nurl = \"https://sam:pw@mcp.example.com\"\n" => [format!("line 4: [[mcp_servers]] entry 1 (\"s\"), url: {NO_LOGIN_IN_URL}")];
    mcp_trust_is_one_of_two:
        mcp_server("trust = \"always\"") => ["line 5: [[mcp_servers]] entry 1 (\"s\"), trust: must be \"ask\" or \"allow\", but it's \"always\" here."];
    mcp_pass_env_holds_variable_names:
        mcp_server("pass_env = [\"GOOD_NAME\", \"BAD-NAME\"]") => ["line 5: [[mcp_servers]] entry 1 (\"s\"), pass_env: item 2 must be the name of an environment variable, such as GITHUB_TOKEN."];
    an_unknown_mcp_key_is_named:
        mcp_server("timeout = 30") => ["line 5: [[mcp_servers]] entry 1 (\"s\"), timeout: isn't a key Wrybill knows."];
}

#[test]
fn an_mcp_server_asks_before_each_call_and_passes_no_environment_by_default() {
    let config = valid(&mcp_server(""));

    assert_eq!(
        config.mcp_servers,
        vec![McpServer {
            id: "s".to_owned(),
            transport: McpTransport::Local {
                command: "server".to_owned(),
                args: vec![],
            },
            trust: McpTrust::Ask,
            pass_env: vec![],
        }]
    );
}

// The whole file at once.

#[test]
fn every_problem_in_a_file_is_reported_in_one_go_in_file_order() {
    let text = "\
version = 1

[defaults]
autonomy = \"atuo\"

[limits]
max_steps_per_task = 0

[[models]]
id = \"m\"
provider = \"anthropic\"
model = \"x\"
cost_tier = \"cheap\"

[storage]
retention_days = \"thirty\"
";

    assert_eq!(
        problems(text),
        [
            "line 4: [defaults] autonomy: must be \"plan\", \"ask\" or \"auto\", but it's \"atuo\" here.",
            "line 7: [limits] max_steps_per_task: must be 1 or more, but it's 0 here.",
            "line 13: [[models]] entry 1 (\"m\"), cost_tier: must be \"low\", \"medium\" or \"high\", but it's \"cheap\" here.",
            "line 16: [storage] retention_days: must be a whole number, 1 or more, but it's text here.",
        ]
    );
}

#[test]
fn no_problem_ever_shows_a_value_that_could_be_a_key() {
    // The same key-shaped text, pasted into every kind of place it could land.
    let texts = [
        hosted_model(&format!("api_key = \"{MARKER}\"")),
        hosted_model(&format!("api_kye = \"{MARKER}\"")),
        hosted_model(&format!("tools = \"{MARKER}\"")),
        hosted_model(&format!("roles = [\"{MARKER}\"]")),
        hosted_model(&format!("api_key = {MARKER}")),
        hosted_model(MARKER),
        hosted_model(&format!("{MARKER} = true")),
        hosted_model(&format!("vision = \"{MARKER}\"")),
        local_model("").replace("127.0.0.1:11434", &format!("{MARKER}@127.0.0.1")),
        section("search", &format!("provider = \"{MARKER}\"")),
        section(
            "search",
            &format!("provider = \"brave\"\napi_key = \"{MARKER}\""),
        ),
        section("defaults", &format!("autonomy = \"{MARKER}\"")),
        section("safety", &format!("workspace_roots = [\"{MARKER}\"]")),
        section(
            "safety",
            &format!("trusted_install_domains = [\"{MARKER}\"]"),
        ),
        section("safety", &format!("dev_hosts = [\"{MARKER} x\"]")),
        mcp_server(&format!("pass_env = [\"{MARKER}\"]")),
        format!("version = 1\n[[models]]\nid = \"{MARKER}\"\nprovider = 1\n"),
        format!("version = \"{MARKER}\"\n"),
        format!("version = 1\n[{MARKER}]\n"),
    ];

    for text in texts {
        let found = problems(&text);

        assert!(
            !found.is_empty(),
            "this config should have a problem:\n{text}"
        );
        for problem in found {
            assert!(
                !problem.contains("MARKER"),
                "a problem shows the value:\n{problem}\nfor this config:\n{text}"
            );
        }
    }
}
