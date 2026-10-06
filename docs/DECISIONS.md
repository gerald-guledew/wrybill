# Wrybill: key decisions

The main design choices and the reason for each, so nobody has to guess why something is the way it is. [SPEC.md](SPEC.md) says what to build; this file says why. To reopen a decision, open an issue with new evidence.

| Area | Decision | Why |
|---|---|---|
| Product | Build on existing LLMs; don't train one | Training costs far too much (N1) |
| Product | No digital twin in v1 | Out of scope for now; it may come later (N8) |
| Product | Open source under the Apache-2.0 licence | So others can benefit (R13). It's a permissive licence with an explicit patent grant, and it lets Wrybill reuse code from other Apache-licensed projects |
| Product | The project is called Wrybill: the command is `wrybill` (never shortened to `wry`), the crates are `wrybill-*` and the data folder is `~/.wrybill` | `wry` is the name of Tauri's WebView library, so the short form would confuse |
| Structure | Core in Rust | Lightweight with no runtime to install, still builds for 2015 Macs, safer than C or C++, and proven for this job by other agent CLIs |
| Structure | Wrybill's own thin agent loop; no heavy agent framework | Lightweight, full control of safety, less churn (N7) |
| Structure | Two binaries (CLI and desktop app) over one shared core | The CLI works without WebKitGTK and writes to the terminal properly on Windows |
| Structure | Desktop app in Tauri v2 with React and TypeScript, not Electron | Small, uses the system webview and shares the Rust core |
| Structure | CLI first; desktop app once the core is stable | Faster to build and test |
| Brains | A `Brain` trait over the `genai` crate | A fast start with many providers, and the crate stays replaceable |
| Brains | Local models run in OpenAI-compatible servers (Ollama, LM Studio, llama.cpp), not inside Wrybill | Keeps Wrybill small |
| Brains | Capability profiles are declared, then probed, then measured | Configs can be wrong, and probes are cheap |
| Brains | Local-only is a mode that must pass its own test; a cloud planner is the default when cloud is allowed; work never moves from local to cloud silently | Honours R5 without crippling R8 |
| Helpers | One agent by default; helpers only when justified | Cost and speed |
| Helpers | Local helpers run in parallel only as far as the config limit, the model server, free memory and a speed probe allow; otherwise one at a time | A fixed "one at a time" wastes capable machines, and on a modest laptop parallel local helpers only slow each other down (SPEC 9.4) |
| Safety | Safety is enforced in code, with Allow, Ask and Deny tiers | Brains can be wrong or tricked |
| Safety | Plan, Ask and Auto autonomy levels, plus task-scoped grants | Fewer, better approval prompts |
| Safety | Taint rules based on the lethal trifecta and the Rule of Two | A principled line against data theft (SPEC 11.7) |
| Safety | Programs run with argument lists by default; allowlisted commands are Allow only in the user's own folders or the sandbox; path checks resolve links | Downloaded repos can run code through their config and hooks, and prefix checks miss symlink escapes (SPEC 11.5) |
| Safety | Reflection and summaries follow the task's privacy and locality rules, and reflection runs only when a task is worth learning from | Stops private content reaching a cloud brain through a side role, and saves tokens (SPEC 8.4) |
| Safety | A task store with interrupt and resume; outside side effects are never blindly retried | A crash shouldn't lose work or repeat a post or payment |
| Safety | A network ledger of Wrybill's own connections, plus a packet-capture test anyone can repeat, instead of packet capture inside Wrybill | What an AI says about where data goes can't be trusted, so destinations are recorded and checkable. Packet capture inside Wrybill would need administrator rights and extra drivers (SPEC 11.10, 11.15, 18) |
| Safety | The fixed, read-only checks behind `wrybill doctor` aren't Guardian actions | Checking the machine is Wrybill's own behaviour, not an action a brain proposed. What the `system.profile` tool passes on is decided in M1 |
| Memory | Learning lives in Markdown plus SQLite; no self-modifying code | Transparent, safe and editable (N2) |
| Memory | Full-text search first; vector search only if evals justify it | No embedding model needed, and `sqlite-vec` is still an alpha release |
| Memory | Agent Skills format for skills; `AGENTS.md` for coding-agent rules | Portable and widely supported |
| Config | TOML config; API keys in the OS keychain | Simple and hard to break; keys stay protected |
| Config | Data in `~/.wrybill` (movable with `WRYBILL_HOME`); task workspaces in `~/Wrybill/workspaces` | The same on every OS and easy to back up; results stay in a visible folder |
| Config | `env:` key references are accepted on every OS, and `wrybill doctor` warns wherever one is used | Servers, containers, CI and SSH sessions often can't reach a keychain (SPEC 11.6) |
| Config | On Linux the keychain is reached through a pure-Rust Secret Service client, not libdbus | The CLI has to start on headless Linux and build as a static `musl` binary (SPEC 6, 14.1) |
| Config | The hidden prompt in `wrybill keys set` uses the `rpassword` crate | It's small, and it does one job on all three OSes |
| Config | Settings are optional with sensible defaults: keys are found by their provider's name, and Wrybill works out a model's roles and abilities itself | The main goal is to be easy and simple for the user (SPEC 1) |
| Browser | Two backends behind one interface (CDP for an installed Chromium browser, WebDriver BiDi for Firefox), always with Wrybill's own profile | Lighter and safer than bundling a browser or using the user's real profile. Chrome 136 and later block automation of the default profile, and Chrome 151 dropped macOS 12, so 2015 Macs need Firefox |
| Platforms | 64-bit only, with a baseline x86-64 CPU target | Covers almost every 2015 laptop without special builds |
| Platforms | Linux GNU builds are made on Ubuntu 24.04, and the static `musl` CLI covers older systems | GitHub removes its Ubuntu 22.04 runners on 17 April 2027. Until then CI also runs the static binaries there (SPEC 14.1) |
| Testing | A starter eval suite (E1 to E11) defines "any task" until real everyday jobs replace it | Milestones need concrete done-when checks from the start (SPEC 18.1) |
| Tooling | CI uses only GitHub's own actions, pinned by commit SHA, and `deny.toml` bans OpenSSL and native-tls | The build gets the same care as the dependencies (SPEC 11.8), and "no OpenSSL" (SPEC 14) is enforced by a tool |
