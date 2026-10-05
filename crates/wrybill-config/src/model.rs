//! The config once it has been checked, with every default filled in
//! (spec 13.3).

use std::path::{Path, PathBuf};

use url::Url;

use crate::keyref::KeyRef;

/// Where task workspaces live when the config doesn't say (spec 15.2).
const DEFAULT_WORKSPACE_ROOT: &str = "Wrybill/workspaces";

/// The hosts that count as local development hosts by default (spec 11.6).
const DEFAULT_DEV_HOSTS: [&str; 5] = ["localhost", "127.0.0.1", "[::1]", "*.localhost", "*.test"];

/// Where a self-hosted SearXNG usually listens (spec 13.3).
pub(crate) const SEARXNG_DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8888";

/// Wrybill's settings: what `config.toml` says, with the built-in default for
/// everything it leaves out.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Which models to prefer, and how much Wrybill may do without asking.
    pub defaults: Defaults,
    /// The limits every task starts with.
    pub limits: Limits,
    /// Whether work may go to cloud brains.
    pub routing: Routing,
    /// The web search provider. `None` until one is set, and web search
    /// stays off.
    pub search: Option<Search>,
    /// Folders, install sources and local development hosts.
    pub safety: Safety,
    /// How long checkpoints, trash and transcripts are kept.
    pub storage: Storage,
    /// Every brain Wrybill may use. Empty on a first run.
    pub models: Vec<Model>,
    /// The MCP servers that add tools.
    pub mcp_servers: Vec<McpServer>,
}

impl Config {
    /// The built-in defaults: what Wrybill runs on when there's no config
    /// file. The home folder is needed for the default workspace folder.
    pub fn defaults(user_home: Option<&Path>) -> Self {
        Self {
            defaults: Defaults::default(),
            limits: Limits::default(),
            routing: Routing::default(),
            search: None,
            safety: Safety::defaults(user_home),
            storage: Storage::default(),
            models: Vec::new(),
            mcp_servers: Vec::new(),
        }
    }
}

/// The `[defaults]` section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Defaults {
    /// The `id` of the model to plan with. `None` lets Wrybill pick.
    pub planner: Option<String>,
    /// The `id` of the model for routine steps. `None` lets Wrybill pick.
    pub worker: Option<String>,
    /// The `id` of the model for summaries. `None` lets Wrybill pick.
    pub summariser: Option<String>,
    /// How much Wrybill may do in a task without asking.
    pub autonomy: Autonomy,
}

/// How much Wrybill may do in a task without asking (spec 11.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Autonomy {
    /// Look, read, search and plan. Change nothing.
    Plan,
    /// Do what's allowed, and wait for approval on the rest.
    #[default]
    Ask,
    /// Also work inside the task workspace without asking.
    Auto,
}

impl Autonomy {
    pub(crate) const CHOICES: [(&'static str, Self); 3] = [
        ("plan", Self::Plan),
        ("ask", Self::Ask),
        ("auto", Self::Auto),
    ];
}

/// The `[limits]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// How many cloud helpers may run at once.
    pub max_parallel_cloud_helpers: u32,
    /// The most calls Wrybill sends to one local or LAN server at once.
    pub max_parallel_calls_per_local_server: u32,
    /// How deep helpers may nest. Always 1 in version 1.
    pub max_helper_depth: u32,
    /// The most steps one task may take.
    pub max_steps_per_task: u32,
    /// The longest one task may run, in minutes.
    pub max_minutes_per_task: u32,
    /// The most tokens one task may use, input plus output, helpers included.
    pub max_tokens_per_task: u64,
    /// The most one task may spend, in US dollars.
    pub max_spend_usd_per_task: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_parallel_cloud_helpers: 3,
            max_parallel_calls_per_local_server: 4,
            max_helper_depth: 1,
            max_steps_per_task: 60,
            max_minutes_per_task: 30,
            max_tokens_per_task: 2_000_000,
            max_spend_usd_per_task: 2.0,
        }
    }
}

/// The `[routing]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    /// `false` allows only local brains and LAN brains marked as trusted.
    pub allow_cloud: bool,
    /// What happens when a task on local brains would need a cloud one.
    pub local_to_cloud: LocalToCloud,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            allow_cloud: true,
            local_to_cloud: LocalToCloud::default(),
        }
    }
}

/// What happens when a task on local brains would need a cloud one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LocalToCloud {
    /// Ask the user first.
    #[default]
    Ask,
    /// Never move the work to the cloud.
    Never,
}

impl LocalToCloud {
    pub(crate) const CHOICES: [(&'static str, Self); 2] =
        [("ask", Self::Ask), ("never", Self::Never)];
}

/// The `[search]` section, once a provider is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    /// The search service.
    pub provider: SearchProvider,
    /// Where its key is kept. When the file doesn't say, this is the key
    /// saved under the provider's name. `None` for SearXNG, which takes none.
    pub api_key: Option<KeyRef>,
    /// The address of a self-hosted SearXNG. `None` for the other providers.
    pub endpoint: Option<Url>,
}

/// A web search service Wrybill can use (spec 10.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchProvider {
    /// Brave Search.
    Brave,
    /// Exa.
    Exa,
    /// Tavily.
    Tavily,
    /// A self-hosted SearXNG.
    Searxng,
}

impl SearchProvider {
    pub(crate) const CHOICES: [(&'static str, Self); 4] = [
        ("brave", Self::Brave),
        ("exa", Self::Exa),
        ("tavily", Self::Tavily),
        ("searxng", Self::Searxng),
    ];

    /// The name the config file and `wrybill keys set` use for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Brave => "brave",
            Self::Exa => "exa",
            Self::Tavily => "tavily",
            Self::Searxng => "searxng",
        }
    }

    /// Whether this service needs an API key. A self-hosted SearXNG doesn't.
    pub fn takes_a_key(self) -> bool {
        !matches!(self, Self::Searxng)
    }
}

/// The `[safety]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Safety {
    /// Folders where task workspaces may live, as full paths.
    pub workspace_roots: Vec<PathBuf>,
    /// Vendor domains that software may be installed from.
    pub trusted_install_domains: Vec<String>,
    /// Hosts that count as local development hosts.
    pub dev_hosts: Vec<String>,
    /// Whether to warn once when the OS no longer gets security updates.
    pub warn_on_unsupported_os: bool,
}

impl Safety {
    /// The defaults. With no known home folder there's nowhere to put the
    /// default workspace folder, so the list is empty.
    pub fn defaults(user_home: Option<&Path>) -> Self {
        Self {
            workspace_roots: user_home
                .map(|home| vec![home.join(DEFAULT_WORKSPACE_ROOT)])
                .unwrap_or_default(),
            trusted_install_domains: Vec::new(),
            dev_hosts: DEFAULT_DEV_HOSTS.map(str::to_owned).to_vec(),
            warn_on_unsupported_os: true,
        }
    }
}

/// The `[storage]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    /// How many days checkpoints, trash and transcripts are kept.
    pub retention_days: u32,
    /// The most disk space they may use, in gigabytes.
    pub retention_max_gb: f64,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            retention_days: 30,
            retention_max_gb: 2.0,
        }
    }
}

/// One `[[models]]` entry: a brain Wrybill may use.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The name the rest of the config and `--brain` use for it.
    pub id: String,
    /// Who serves it.
    pub provider: Provider,
    /// The provider's or server's own name for the model.
    pub model: String,
    /// What it's meant for. `None` lets Wrybill decide.
    pub roles: Option<Vec<Role>>,
    /// Where its key is kept. For a hosted provider with no `api_key` line,
    /// this is the key saved under the provider's name.
    pub api_key: Option<KeyRef>,
    /// The server's address, for an OpenAI-compatible server.
    pub endpoint: Option<Url>,
    /// Where it runs. Hosted providers are always cloud.
    pub locality: Locality,
    /// How it's asked to call tools.
    pub tools: ToolCalling,
    /// Whether it accepts images. `None` until a probe has worked it out.
    pub vision: Option<bool>,
    /// Its context size in tokens. `None` until a probe has worked it out.
    pub context_window: Option<u64>,
    /// The free memory a local model needs, in gigabytes.
    pub min_free_ram_gb: Option<f64>,
    /// Whether a LAN model may be used for private work.
    pub trusted_for_private: bool,
    /// A rough cost band, for when exact prices aren't given.
    pub cost_tier: Option<CostTier>,
    /// The price of a million input tokens, in US dollars.
    pub price_in_usd_per_mtok: Option<f64>,
    /// The price of a million output tokens, in US dollars.
    pub price_out_usd_per_mtok: Option<f64>,
    /// `false` switches the model off without deleting its entry.
    pub enabled: bool,
    /// Free text for the person who owns the file.
    pub notes: Option<String>,
}

/// Who serves a model (spec 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Anthropic (Claude).
    Anthropic,
    /// OpenAI.
    Openai,
    /// Google (Gemini).
    Gemini,
    /// OpenRouter.
    Openrouter,
    /// Any server that speaks the OpenAI-compatible API, such as Ollama, LM
    /// Studio or llama.cpp.
    OpenaiCompatible,
}

impl Provider {
    pub(crate) const CHOICES: [(&'static str, Self); 5] = [
        ("anthropic", Self::Anthropic),
        ("openai", Self::Openai),
        ("gemini", Self::Gemini),
        ("openrouter", Self::Openrouter),
        ("openai-compatible", Self::OpenaiCompatible),
    ];

    /// The name the config file and `wrybill keys set` use for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
            Self::Openrouter => "openrouter",
            Self::OpenaiCompatible => "openai-compatible",
        }
    }

    /// Whether this is one of the four hosted providers, whose address
    /// Wrybill already knows and whose models always run in the cloud.
    pub fn is_hosted(self) -> bool {
        !matches!(self, Self::OpenaiCompatible)
    }
}

/// What a model is meant for (spec 8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Planning a task.
    Planner,
    /// Carrying out routine steps.
    Worker,
    /// Writing summaries.
    Summariser,
}

impl Role {
    pub(crate) const CHOICES: [(&'static str, Self); 3] = [
        ("planner", Self::Planner),
        ("worker", Self::Worker),
        ("summariser", Self::Summariser),
    ];

    /// The name the config file uses for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Worker => "worker",
            Self::Summariser => "summariser",
        }
    }
}

/// Where a brain really runs (spec 8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locality {
    /// On someone else's servers.
    Cloud,
    /// On another computer on the same network.
    Lan,
    /// On this computer.
    Local,
}

impl Locality {
    pub(crate) const CHOICES: [(&'static str, Self); 3] = [
        ("cloud", Self::Cloud),
        ("lan", Self::Lan),
        ("local", Self::Local),
    ];

    /// The name the config file uses for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Lan => "lan",
            Self::Local => "local",
        }
    }
}

/// How a model is asked to call tools (spec 8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCalling {
    /// The model's own tool-calling feature.
    Native,
    /// Tools described in the prompt, with one strict JSON block back.
    Prompted,
    /// The model isn't given tools.
    None,
}

impl ToolCalling {
    pub(crate) const CHOICES: [(&'static str, Self); 3] = [
        ("native", Self::Native),
        ("prompted", Self::Prompted),
        ("none", Self::None),
    ];
}

/// A rough cost band for a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostTier {
    /// Cheap.
    Low,
    /// In between.
    Medium,
    /// Expensive.
    High,
}

impl CostTier {
    pub(crate) const CHOICES: [(&'static str, Self); 3] = [
        ("low", Self::Low),
        ("medium", Self::Medium),
        ("high", Self::High),
    ];
}

/// One `[[mcp_servers]]` entry: a server that adds tools (spec 10.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    /// The name Wrybill uses for it.
    pub id: String,
    /// How Wrybill reaches it.
    pub transport: McpTransport,
    /// Whether its tools need approval each time.
    pub trust: McpTrust,
    /// The environment variables a local server may see.
    pub pass_env: Vec<String>,
}

/// How Wrybill reaches an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpTransport {
    /// A program Wrybill starts on this computer.
    Local {
        /// The program to run.
        command: String,
        /// Its arguments.
        args: Vec<String>,
    },
    /// A server at a web address.
    Remote {
        /// The server's address.
        url: Url,
    },
}

/// Whether an MCP server's tools need approval each time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpTrust {
    /// Ask before each tool call.
    #[default]
    Ask,
    /// Allow its tool calls without asking.
    Allow,
}

impl McpTrust {
    pub(crate) const CHOICES: [(&'static str, Self); 2] =
        [("ask", Self::Ask), ("allow", Self::Allow)];
}
