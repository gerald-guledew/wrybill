//! The examples in spec 13.2, read straight out of `docs/SPEC.md`.
//!
//! Nothing here is a copy, so the spec and the code can't drift apart
//! quietly: if either changes, these tests say so.

use wrybill_config::{
    Autonomy, Config, KeyRef, Limits, Locality, McpTransport, McpTrust, Provider, Role,
    SearchProvider, Storage, ToolCalling, parse,
};

use crate::support::home;

const SPEC: &str = include_str!("../../../../docs/SPEC.md");

/// Every `toml` block in section 13.2, in order.
fn examples() -> Vec<&'static str> {
    let (_, from_13_2) = SPEC
        .split_once("\n### 13.2 Example\n")
        .expect("the spec has a section 13.2");
    let (section, _) = from_13_2
        .split_once("\n### 13.3 ")
        .expect("section 13.3 follows 13.2");

    let mut blocks = Vec::new();
    let mut rest = section;
    while let Some((_, after_start)) = rest.split_once("```toml\n") {
        let (block, after_end) = after_start
            .split_once("\n```")
            .expect("every toml block is closed");
        blocks.push(block);
        rest = after_end;
    }
    blocks
}

fn example(index: usize) -> Config {
    let valid = parse(examples()[index], Some(&home())).expect("a valid example");
    valid.config
}

#[test]
fn section_13_2_has_the_smallest_config_and_the_full_example() {
    assert_eq!(examples().len(), 2);
}

#[test]
fn every_example_is_a_valid_config_with_nothing_to_warn_about() {
    for (index, text) in examples().into_iter().enumerate() {
        match parse(text, Some(&home())) {
            Ok(valid) => assert!(
                valid.warnings.is_empty(),
                "example {} has warnings: {:#?}",
                index + 1,
                valid.warnings
            ),
            Err(invalid) => panic!(
                "example {} in spec 13.2 isn't a valid config: {:#?}",
                index + 1,
                invalid
                    .errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

#[test]
fn the_smallest_example_is_one_model_and_wrybill_works_out_the_rest() {
    let config = example(0);

    assert_eq!(config.models.len(), 1);
    let model = &config.models[0];
    assert_eq!(model.id, "claude");
    assert_eq!(model.provider, Provider::Anthropic);
    assert_eq!(model.model, "claude-sonnet-5-5");
    // No `api_key` line: the key saved by `wrybill keys set anthropic`.
    assert_eq!(model.api_key, KeyRef::keychain("anthropic"));
    // No `roles` or `vision` line: Wrybill decides.
    assert_eq!(model.roles, None);
    assert_eq!(model.vision, None);
    assert_eq!(model.locality, Locality::Cloud);
    assert_eq!(model.tools, ToolCalling::Native);
    assert!(model.enabled);

    // Everything else is the built-in default.
    let defaults = Config::defaults(Some(&home()));
    assert_eq!(
        Config {
            models: vec![],
            ..config
        },
        defaults
    );
}

#[test]
fn the_full_example_reads_the_way_the_spec_describes_it() {
    let config = example(1);

    assert_eq!(config.defaults.planner.as_deref(), Some("claude-sonnet"));
    assert_eq!(config.defaults.worker.as_deref(), Some("claude-haiku"));
    assert_eq!(config.defaults.summariser.as_deref(), Some("local-small"));
    assert_eq!(config.defaults.autonomy, Autonomy::Ask);

    assert!(config.routing.allow_cloud);

    let search = config.search.expect("the example sets a search provider");
    assert_eq!(search.provider, SearchProvider::Brave);
    assert_eq!(search.api_key, KeyRef::keychain("brave"));
    assert_eq!(search.endpoint, None);

    assert_eq!(
        config.safety.workspace_roots,
        vec![
            home().join("Wrybill").join("workspaces"),
            home().join("Projects")
        ]
    );
    assert_eq!(
        config.safety.trusted_install_domains,
        ["sh.rustup.rs", "nodejs.org", "www.python.org"]
    );

    let ids: Vec<&str> = config
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "claude-sonnet",
            "claude-haiku",
            "openai-main",
            "gemini-main",
            "local-small",
            "local-old-mac",
            "lan-strong"
        ]
    );

    let sonnet = &config.models[0];
    assert_eq!(sonnet.roles, Some(vec![Role::Planner, Role::Worker]));
    assert_eq!(sonnet.api_key, KeyRef::keychain("anthropic"));
    assert_eq!(sonnet.vision, Some(true));

    let local = &config.models[4];
    assert_eq!(local.provider, Provider::OpenaiCompatible);
    assert_eq!(local.locality, Locality::Local);
    assert_eq!(local.api_key, None);
    assert_eq!(local.context_window, Some(16384));
    assert_eq!(local.min_free_ram_gb, Some(6.0));
    assert_eq!(
        local.endpoint.as_ref().map(|url| url.as_str()),
        Some("http://127.0.0.1:11434/v1")
    );

    let old_mac = &config.models[5];
    assert_eq!(old_mac.tools, ToolCalling::Prompted);

    let lan = &config.models[6];
    assert_eq!(lan.locality, Locality::Lan);
    assert!(lan.trusted_for_private);

    assert_eq!(config.mcp_servers.len(), 1);
    let server = &config.mcp_servers[0];
    assert_eq!(server.id, "example-server");
    assert_eq!(
        server.transport,
        McpTransport::Local {
            command: "<path-to-server>".to_owned(),
            args: vec![],
        }
    );
    assert_eq!(server.trust, McpTrust::Ask);
    assert!(server.pass_env.is_empty());
}

#[test]
fn the_full_example_shows_the_built_in_defaults_for_limits_storage_and_dev_hosts() {
    // Spec 13.3 gives these defaults by pointing at the example, so the
    // example's values and the code's defaults have to be the same.
    let config = example(1);
    let defaults = Config::defaults(Some(&home()));

    assert_eq!(config.limits, Limits::default());
    assert_eq!(config.storage, Storage::default());
    assert_eq!(config.safety.dev_hosts, defaults.safety.dev_hosts);
    assert_eq!(
        config.safety.warn_on_unsupported_os,
        defaults.safety.warn_on_unsupported_os
    );
    assert_eq!(config.routing, defaults.routing);
}
