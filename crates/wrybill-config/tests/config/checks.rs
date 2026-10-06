//! The checks that look across keys: every start-up error and warning at
//! the end of spec 13.3.

use crate::support::{hosted_model, local_model, problem_tests, problems, valid, warnings};

const NO_PLANNER_LEFT: &str = "leaves no model that can plan. Every model that could is a cloud model, or a LAN model that isn't marked `trusted_for_private`. Add a local model, mark a LAN model as trusted, or set `allow_cloud = true`.";

/// A `[[models]]` entry for a hosted provider, six lines long with the blank
/// line it starts with. `extra` holds any further lines.
fn hosted(id: &str, extra: &str) -> String {
    format!("\n[[models]]\nid = \"{id}\"\nprovider = \"anthropic\"\nmodel = \"x\"\n{extra}\n")
}

/// A `[[models]]` entry for a server of your own, eight lines long with the
/// blank line it starts with. `extra` holds any further lines.
fn served(id: &str, locality: &str, extra: &str) -> String {
    format!(
        "\n[[models]]\nid = \"{id}\"\nprovider = \"openai-compatible\"\nmodel = \"x\"\nendpoint = \"http://10.0.0.2:8080/v1\"\nlocality = \"{locality}\"\n{extra}\n"
    )
}

/// A config that starts with `[defaults]`. Its first entry is on line 3.
fn with_defaults(defaults: &str, models: &[String]) -> String {
    format!("version = 1\n[defaults]\n{defaults}\n{}", models.concat())
}

/// A config that starts with `allow_cloud = false`, on line 3.
fn without_cloud(models: &[String]) -> String {
    format!(
        "version = 1\n[routing]\nallow_cloud = false\n{}",
        models.concat()
    )
}

// A `[defaults]` entry names a model that doesn't exist, is disabled, lists
// roles that don't include that one, or is ruled out by `allow_cloud = false`.

problem_tests! {
    a_default_cannot_name_a_model_when_there_are_none:
        with_defaults("planner = \"big\"", &[]) => ["line 3: [defaults] planner: no model has the id \"big\". This file has no models yet."];
    a_default_has_to_name_a_model_that_exists:
        with_defaults("planner = \"bigg\"", &[hosted("big", "")]) => ["line 3: [defaults] planner: no model has the id \"bigg\". The only model here is \"big\"."];
    a_missing_model_is_answered_with_the_ones_that_exist:
        with_defaults("worker = \"tiny\"", &[hosted("big", ""), hosted("mid", ""), served("small", "local", "")]) => ["line 3: [defaults] worker: no model has the id \"tiny\". The models here are \"big\", \"mid\" and \"small\"."];
    a_default_cannot_name_a_model_that_is_switched_off:
        with_defaults("worker = \"small\"", &[hosted("small", "enabled = false")]) => ["line 3: [defaults] worker: the model \"small\" is switched off with `enabled = false`. Switch it on, or name another model."];
    the_default_planner_has_to_be_able_to_plan:
        with_defaults("planner = \"small\"", &[hosted("small", "roles = [\"worker\", \"summariser\"]")]) => ["line 3: [defaults] planner: the model \"small\" lists its roles, and \"planner\" isn't one of them. Add it to that model's `roles`, or name another model."];
    the_default_worker_has_to_be_a_worker:
        with_defaults("worker = \"big\"", &[hosted("big", "roles = [\"planner\"]")]) => ["line 3: [defaults] worker: the model \"big\" lists its roles, and \"worker\" isn't one of them. Add it to that model's `roles`, or name another model."];
    the_default_summariser_has_to_be_a_summariser:
        with_defaults("summariser = \"big\"", &[hosted("big", "roles = [\"planner\"]")]) => ["line 3: [defaults] summariser: the model \"big\" lists its roles, and \"summariser\" isn't one of them. Add it to that model's `roles`, or name another model."];
    a_default_cannot_name_a_cloud_model_when_cloud_is_off:
        format!("version = 1\n[routing]\nallow_cloud = false\n[defaults]\nworker = \"big\"\n{}{}", hosted("big", "roles = [\"worker\"]"), served("home", "local", ""))
            => ["line 5: [defaults] worker: the model \"big\" runs in the cloud, and `allow_cloud = false` rules that out."];
    a_default_cannot_name_an_untrusted_lan_model_when_cloud_is_off:
        format!("version = 1\n[routing]\nallow_cloud = false\n[defaults]\nworker = \"shed\"\n{}{}", served("shed", "lan", "roles = [\"worker\"]"), served("home", "local", ""))
            => ["line 5: [defaults] worker: the model \"shed\" is a LAN model that isn't marked `trusted_for_private`, and `allow_cloud = false` rules that out."];
    a_default_with_several_things_wrong_reports_each_one:
        with_defaults("planner = \"small\"", &[hosted("small", "roles = [\"worker\"]\nenabled = false")]) => [
            "line 3: [defaults] planner: the model \"small\" is switched off with `enabled = false`. Switch it on, or name another model.",
            "line 3: [defaults] planner: the model \"small\" lists its roles, and \"planner\" isn't one of them. Add it to that model's `roles`, or name another model.",
        ];
}

#[test]
fn a_default_can_name_a_model_that_lists_no_roles() {
    let text = with_defaults(
        "planner = \"any\"\nworker = \"any\"\nsummariser = \"any\"",
        &[hosted("any", "")],
    );

    assert_eq!(valid(&text).defaults.planner.as_deref(), Some("any"));
}

#[test]
fn a_default_can_name_a_trusted_lan_model_or_a_local_one_when_cloud_is_off() {
    let text = format!(
        "version = 1\n[routing]\nallow_cloud = false\n[defaults]\nplanner = \"shed\"\nworker = \"home\"\n{}{}",
        served("shed", "lan", "trusted_for_private = true"),
        served("home", "local", "")
    );

    let config = valid(&text);

    assert_eq!(config.defaults.planner.as_deref(), Some("shed"));
    assert_eq!(config.defaults.worker.as_deref(), Some("home"));
}

// `allow_cloud = false` when the only enabled models that can plan are ones
// it rules out. A model with no `roles` line counts as able to plan.

problem_tests! {
    cloud_cannot_be_off_when_only_cloud_models_can_plan:
        without_cloud(&[hosted("big", "roles = [\"planner\"]")]) => [format!("line 3: [routing] allow_cloud: {NO_PLANNER_LEFT}")];
    cloud_cannot_be_off_when_the_only_other_planner_is_an_untrusted_lan_model:
        without_cloud(&[hosted("big", ""), served("shed", "lan", "")]) => [format!("line 3: [routing] allow_cloud: {NO_PLANNER_LEFT}")];
    a_model_with_no_roles_line_counts_as_able_to_plan:
        without_cloud(&[hosted("big", ""), served("home", "local", "roles = [\"summariser\"]")]) => [format!("line 3: [routing] allow_cloud: {NO_PLANNER_LEFT}")];
    a_local_planner_that_is_switched_off_does_not_count:
        without_cloud(&[hosted("big", ""), served("home", "local", "enabled = false")]) => [format!("line 3: [routing] allow_cloud: {NO_PLANNER_LEFT}")];
    a_cloud_model_on_a_server_of_your_own_is_still_a_cloud_model:
        without_cloud(&[served("rented", "cloud", "")]) => [format!("line 3: [routing] allow_cloud: {NO_PLANNER_LEFT}")];
}

#[test]
fn cloud_can_be_off_when_a_local_model_can_plan() {
    let text = without_cloud(&[hosted("big", ""), served("home", "local", "")]);

    assert!(!valid(&text).routing.allow_cloud);
}

#[test]
fn cloud_can_be_off_when_a_trusted_lan_model_can_plan() {
    let text = without_cloud(&[
        hosted("big", ""),
        served("shed", "lan", "trusted_for_private = true"),
    ]);

    assert!(!valid(&text).routing.allow_cloud);
}

#[test]
fn cloud_can_be_off_with_no_models_at_all() {
    // The first-run state: nothing to plan with yet, and nothing contradicts.
    assert!(!valid(&without_cloud(&[])).routing.allow_cloud);
}

#[test]
fn cloud_can_be_off_when_no_model_is_meant_to_plan() {
    let text = without_cloud(&[hosted("notes", "roles = [\"summariser\"]")]);

    assert!(!valid(&text).routing.allow_cloud);
}

#[test]
fn a_cloud_model_that_is_switched_off_contradicts_nothing() {
    let text = without_cloud(&[hosted("big", "enabled = false")]);

    assert!(!valid(&text).routing.allow_cloud);
}

// Two models, or two MCP servers, share an `id`.

problem_tests! {
    two_models_cannot_share_an_id:
        format!("version = 1\n{}{}{}", hosted("big", ""), hosted("small", ""), hosted("big", ""))
            => ["line 16: [[models]] entry 3 (\"big\"), id: entry 1 already uses this id. Give each model its own."];
    two_mcp_servers_cannot_share_an_id:
        "version = 1\n[[mcp_servers]]\nid = \"files\"\ncommand = \"a\"\n[[mcp_servers]]\nid = \"files\"\ncommand = \"b\"\n"
            => ["line 6: [[mcp_servers]] entry 2 (\"files\"), id: entry 1 already uses this id. Give each MCP server its own."];
}

#[test]
fn a_model_and_an_mcp_server_can_share_an_id() {
    let text = format!(
        "version = 1\n{}\n[[mcp_servers]]\nid = \"files\"\ncommand = \"a\"\n",
        hosted("files", "")
    );

    let config = valid(&text);

    assert_eq!(config.models[0].id, config.mcp_servers[0].id);
}

// A hosted provider is marked `"local"` or `"lan"`, or a cloud model is
// marked `trusted_for_private`.

problem_tests! {
    a_hosted_provider_cannot_be_marked_local:
        hosted_model("locality = \"local\"") => ["line 6: [[models]] entry 1 (\"m\"), locality: \"anthropic\" is a hosted provider, so its models always run in the cloud. Remove this line, or use the provider \"openai-compatible\" for a model you run yourself."];
    a_hosted_provider_cannot_be_marked_lan:
        hosted_model("locality = \"lan\"").replace("anthropic", "openrouter") => ["line 6: [[models]] entry 1 (\"m\"), locality: \"openrouter\" is a hosted provider, so its models always run in the cloud. Remove this line, or use the provider \"openai-compatible\" for a model you run yourself."];
    a_hosted_model_cannot_be_trusted_for_private_work:
        hosted_model("trusted_for_private = true") => ["line 6: [[models]] entry 1 (\"m\"), trusted_for_private: only a LAN model can be trusted for private work, and this one runs in the cloud. Remove this line."];
    a_cloud_model_on_a_server_of_your_own_cannot_be_trusted_for_private_work:
        format!("version = 1\n{}", served("rented", "cloud", "trusted_for_private = true")) => ["line 9: [[models]] entry 1 (\"rented\"), trusted_for_private: only a LAN model can be trusted for private work, and this one runs in the cloud. Remove this line."];
}

#[test]
fn a_hosted_provider_can_say_it_is_cloud() {
    assert_eq!(problems(&hosted_model("locality = \"cloud\"")), [""; 0]);
}

#[test]
fn a_cloud_model_can_say_it_is_not_trusted_for_private_work() {
    let text = hosted_model("trusted_for_private = false");

    assert_eq!(problems(&text), [""; 0]);
    assert_eq!(warnings(&text), [""; 0]);
}

// The rules in the tables: an `endpoint` on a hosted provider, and how the
// `[search]` keys fit the provider.

problem_tests! {
    a_hosted_provider_takes_no_endpoint:
        hosted_model("endpoint = \"https://proxy.example.com/v1\"") => ["line 6: [[models]] entry 1 (\"m\"), endpoint: \"anthropic\" is a hosted provider, and Wrybill already knows its address. Remove this line. To use a server at an address of your own, set the provider to \"openai-compatible\"."];
    search_keys_need_a_provider:
        "version = 1\n[search]\napi_key = \"keychain:wrybill/brave\"\n" => ["line 2: [search]: has other keys but no `provider`. Add `provider`, set to \"brave\", \"exa\", \"tavily\" or \"searxng\"."];
    only_searxng_takes_an_endpoint:
        "version = 1\n[search]\nprovider = \"brave\"\nendpoint = \"http://127.0.0.1:8888\"\n" => ["line 4: [search] endpoint: only \"searxng\" takes an `endpoint`. Wrybill already knows the address of \"brave\". Remove this line."];
    searxng_takes_no_key:
        "version = 1\n[search]\nprovider = \"searxng\"\napi_key = \"keychain:wrybill/searxng\"\n" => ["line 4: [search] api_key: \"searxng\" takes no key. Remove this line."];
    a_wrong_search_provider_is_reported_once:
        "version = 1\n[search]\nprovider = \"google\"\nendpoint = \"http://127.0.0.1:8888\"\n" => ["line 3: [search] provider: must be \"brave\", \"exa\", \"tavily\" or \"searxng\", but it's \"google\" here."];
}

// Two misplaced keys are only warnings.

#[test]
fn min_free_ram_gb_on_a_lan_model_is_a_warning() {
    let text = format!(
        "version = 1\n{}",
        served("shed", "lan", "min_free_ram_gb = 8")
    );

    assert_eq!(problems(&text), [""; 0]);
    assert_eq!(
        warnings(&text),
        [
            "line 9: [[models]] entry 1 (\"shed\"), min_free_ram_gb: only matters for a model on this computer, so it has no effect on this LAN one."
        ]
    );
}

#[test]
fn min_free_ram_gb_on_a_cloud_model_is_a_warning() {
    let text = hosted_model("min_free_ram_gb = 8");

    assert_eq!(problems(&text), [""; 0]);
    assert_eq!(
        warnings(&text),
        [
            "line 6: [[models]] entry 1 (\"m\"), min_free_ram_gb: only matters for a model on this computer, so it has no effect on this cloud one."
        ]
    );
}

#[test]
fn trusted_for_private_on_a_local_model_is_a_warning_whatever_it_says() {
    for value in ["true", "false"] {
        let text = local_model(&format!("trusted_for_private = {value}"));

        assert_eq!(problems(&text), [""; 0], "{value}");
        assert_eq!(
            warnings(&text),
            [
                "line 8: [[models]] entry 1 (\"m\"), trusted_for_private: isn't needed on a local model. Local models can always be used for private work."
            ],
            "{value}"
        );
    }
}

#[test]
fn a_config_with_only_warnings_is_still_used() {
    let config = valid(&hosted_model("min_free_ram_gb = 8"));

    assert_eq!(config.models[0].min_free_ram_gb, Some(8.0));
}

#[test]
fn the_keys_in_their_right_places_warn_about_nothing() {
    let text = format!(
        "version = 1\n{}{}",
        served("home", "local", "min_free_ram_gb = 6"),
        served("shed", "lan", "trusted_for_private = true")
    );

    assert_eq!(problems(&text), [""; 0]);
    assert_eq!(warnings(&text), [""; 0]);
}

#[test]
fn warnings_are_kept_when_there_are_problems_too() {
    let text = hosted_model("min_free_ram_gb = 8\ntools = \"yes\"");

    assert_eq!(problems(&text).len(), 1);
    assert_eq!(warnings(&text).len(), 1);
}

// All six start-up errors that spec 13.3 lists, in one file.

#[test]
fn every_listed_start_up_error_is_reported_in_one_go() {
    let text = "\
version = 1

[defaults]
planner = \"nobody\"

[routing]
allow_cloud = false

[[models]]
id = \"big\"
provider = \"anthropic\"
model = \"x\"
locality = \"local\"
api_key = \"sk-test-MARKER-0123456789abcdef\"

[[models]]
id = \"big\"
provider = \"openai\"
model = \"y\"
trusted_for_private = true

[[mcp_servers]]
id = \"files\"
command = \"files-server\"
url = \"https://mcp.example.com\"

[[mcp_servers]]
id = \"files\"
";

    assert_eq!(
        problems(text),
        [
            // A `[defaults]` entry names a model that doesn't exist.
            "line 4: [defaults] planner: no model has the id \"nobody\". The only model here is \"big\".",
            // `allow_cloud = false` when only cloud models can plan.
            &format!("line 7: [routing] allow_cloud: {NO_PLANNER_LEFT}"),
            // A hosted provider is marked "local".
            "line 13: [[models]] entry 1 (\"big\"), locality: \"anthropic\" is a hosted provider, so its models always run in the cloud. Remove this line, or use the provider \"openai-compatible\" for a model you run yourself.",
            // An `api_key` holds a key instead of a reference.
            "line 14: [[models]] entry 1 (\"big\"), api_key: must point to a key, not hold one. Write `keychain:wrybill/<name>` or `env:VARIABLE_NAME`. The value isn't shown here in case it's a real key. If it is, take it out of this file and run `wrybill keys set anthropic`. This line can then go: Wrybill finds that key by itself.",
            // Two models share an `id`.
            "line 17: [[models]] entry 2 (\"big\"), id: entry 1 already uses this id. Give each model its own.",
            // A cloud model is marked `trusted_for_private`.
            "line 20: [[models]] entry 2 (\"big\"), trusted_for_private: only a LAN model can be trusted for private work, and this one runs in the cloud. Remove this line.",
            // An MCP server has both `command` and `url`.
            "line 22: [[mcp_servers]] entry 1 (\"files\"): has both `command` and `url`. Keep `command` for a server on this computer, or `url` for a remote one.",
            // An MCP server has neither.
            "line 27: [[mcp_servers]] entry 2 (\"files\"): needs either a `command`, for a server on this computer, or a `url`, for a remote one.",
            // Two MCP servers share an `id`.
            "line 28: [[mcp_servers]] entry 2 (\"files\"), id: entry 1 already uses this id. Give each MCP server its own.",
        ]
    );
}
