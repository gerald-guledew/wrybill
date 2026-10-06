# Wrybill: rules for AI coding agents

Wrybill is a secure, lightweight, LLM-agnostic, adaptive AI agent for laptops, being built in Rust with a CLI and a Tauri v2 desktop app. It's open source under Apache-2.0. The specification is `docs/SPEC.md`. It is the source of truth: if this file, a chat, or your own judgement disagrees with it, the spec wins until the maintainer changes it.

**Current milestone:** M0 Foundations. Its brief is `docs/milestones/M0.md`.

## Before you start

- Read the current milestone brief, then Part A of `docs/SPEC.md` (sections 1 to 4) and the sections the brief points to. Read section 11 (Guardian) before working on anything that runs commands, touches files or talks to the network. The spec is long, so read the sections you need rather than the whole file.
- Work on one milestone task at a time. Write a short plan and wait for an OK from the person you're working with before writing code.
- Check `docs/DECISIONS.md` before reopening a settled choice. When a change settles a design question, adds a notable dependency or borrows from another project, propose a line for that file, and keep the licence notices of any borrowed code.
- Don't edit `docs/SPEC.md` yourself. If something in it looks wrong, unclear or out of date, stop and propose the change.
- Wrybill's main goal is to make things easy and simple for its user (spec section 1). When a choice comes up, prefer the option where Wrybill works it out itself, asks only for real decisions, and keeps settings optional with sensible defaults.

## Hard rules

1. Never change Part A or loosen the Guardian's defaults without the maintainer's explicit approval.
2. Brains never touch the laptop. Every action goes through the Guardian, front doors hold no business logic, and tools never call brains.
3. Wrybill never modifies its own source code or binaries at runtime. Learning changes data only (spec 12.8).
4. No heavy frameworks or runtimes inside Wrybill: no LangChain-style frameworks, no Electron, and no Python or Node.js runtime. Node is fine as a build tool for the desktop UI.
5. Prefer the standard library, then small, well-maintained crates with MIT or Apache-2.0 licences. Justify every new dependency in the commit or PR.
6. Cross-platform by default: no hard-coded paths or shells, and every OS-specific branch is tested or clearly marked. Targets include Intel Macs on macOS 11 and 12, and release builds use the baseline x86-64 target (spec 14.1).
7. No `unsafe` without a comment explaining why, and a test.
8. Secrets and personal data never go into code, tests, logs, fixtures or prompts. API keys live in the OS keychain. This repository is public.
9. Names: the command is `wrybill` (never shortened to `wry`, which is Tauri's WebView library), crates are `wrybill-*`, the data folder is `~/.wrybill` (movable with `WRYBILL_HOME`), and Wrybill's own environment variables start with `WRYBILL_`.

## Every change must pass

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
```

- Every Guardian rule ships with tests, including red-team cases (spec section 18).
- No live API calls in tests. Use the mock brain and recorded fixtures.

## Where things will live

M0 creates the workspace. Folders appear as their milestones need them.

```
crates/wrybill-core/       protocol, Conductor, agent loop, context builder, verifier
crates/wrybill-brains/     Brain trait, providers, model registry, probes, router
crates/wrybill-tools/      shell, files, system profile, packages, web, browser, MCP client
crates/wrybill-guardian/   tiers, autonomy levels, grants, taint, classifier, sandbox, audit log, checkpoints
crates/wrybill-memory/     preferences, lessons, skills, stats, retrieval, improvement loop
crates/wrybill-store/      SQLite: task store, indexes, migrations
crates/wrybill-config/     config loading and validation, paths, keychain
apps/wrybill-cli/          clap + ratatui
apps/wrybill-desktop/      Tauri v2 + React and TypeScript
prompts/                   Wrybill's own instructions, versioned like code
evals/                     task suites with pass/fail checks (spec 18.1)
tests/                     integration tests and the red-team suite
docs/                      SPEC.md, DECISIONS.md, milestones/, compatibility.md, benchmarks.md, verify-privacy.md
```

## When you finish

- Keep commits small and reviewable, with clear messages.
- Report what you tested, what you didn't, and anything in the spec that looks wrong.
- Never present a target as a measured result.
- Guardian changes need two reviews before merging: one by the contributor (the person you're working with), with the help of a coding agent that didn't write the change, and one by the maintainer.

## Writing style

Docs and user-facing messages use plain, friendly English in the spec (summarise, behaviour, licence).
