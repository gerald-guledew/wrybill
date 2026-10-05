//! Helpers shared by the config tests.

use std::path::PathBuf;

use wrybill_config::{Config, parse};

/// A home folder that is a full path on whichever OS the tests run on.
pub fn home() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Users\sam")
    } else {
        PathBuf::from("/home/sam")
    }
}

/// The problems a config has, each as the text its owner would see.
pub fn problems(text: &str) -> Vec<String> {
    match parse(text, Some(&home())) {
        Ok(_) => Vec::new(),
        Err(invalid) => invalid.errors.iter().map(ToString::to_string).collect(),
    }
}

/// The config, which has to be valid.
pub fn valid(text: &str) -> Config {
    match parse(text, Some(&home())) {
        Ok(valid) => valid.config,
        Err(_) => panic!("expected a valid config, but found: {:#?}", problems(text)),
    }
}

/// A config with one section. The body starts on line 3.
pub fn section(name: &str, body: &str) -> String {
    format!("version = 1\n[{name}]\n{body}\n")
}

/// A config with one model from a hosted provider. `extra` starts on line 6.
pub fn hosted_model(extra: &str) -> String {
    format!(
        "version = 1\n[[models]]\nid = \"m\"\nprovider = \"anthropic\"\nmodel = \"x\"\n{extra}\n"
    )
}

/// A config with one model on a local server. `extra` starts on line 8.
pub fn local_model(extra: &str) -> String {
    format!(
        "version = 1\n[[models]]\nid = \"m\"\nprovider = \"openai-compatible\"\nmodel = \"x\"\nendpoint = \"http://127.0.0.1:11434/v1\"\nlocality = \"local\"\n{extra}\n"
    )
}

/// A config with one MCP server on this computer. `extra` starts on line 5.
pub fn mcp_server(extra: &str) -> String {
    format!("version = 1\n[[mcp_servers]]\nid = \"s\"\ncommand = \"server\"\n{extra}\n")
}

/// Declares one test per case: a config, and exactly the problems it has.
macro_rules! problem_tests {
    ($($name:ident: $config:expr => [$($problem:expr),* $(,)?];)+) => {
        $(
            #[test]
            fn $name() {
                let config: String = $config.into();
                let expected: Vec<String> = vec![$($problem.to_owned()),*];
                assert_eq!(
                    $crate::support::problems(&config),
                    expected,
                    "for this config:\n{config}"
                );
            }
        )+
    };
}
pub(crate) use problem_tests;
