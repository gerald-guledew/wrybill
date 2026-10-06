# Contributing to Wrybill

Kia ora, and thanks for stopping by! Wrybill is an adaptive AI agent for the laptop you already own, and it's being built in the open. Whether you've got a question, an idea, an old laptop to test on or some Rust to write, there's a way to help.

## Where things stand

Wrybill is in its early days. The design is in [docs/SPEC.md](docs/SPEC.md), and the first milestone has laid the foundations (see its brief, [docs/milestones/M0.md](docs/milestones/M0.md)). There's no release yet, so the most useful help right now is:

- **Reading the spec** and opening an issue when something looks wrong, unclear or missing. Fresh eyes catch the most.
- **Sharing what you need an agent to do.** Real jobs become evals (section 18.1 of the spec), and evals decide what "works" means.
- **Offering test hardware.** A 2015 MacBook, a Windows 10 laptop or an old ThinkPad running Linux is gold for checking that Wrybill stays light (section 17). [docs/compatibility.md](docs/compatibility.md) says how to run `wrybill doctor` on it and send in what it prints.
- **Code.** Look for issues labelled `good first issue`.

## Before you start

- **The spec is the source of truth.** Read Part A (sections 1 to 4) and the sections your change touches. If your change disagrees with the spec, open an issue to change the spec first.
- **Talk before big changes.** For anything bigger than a small fix, open an issue and agree on the approach before you write code. It saves everyone time.
- **Two areas need the maintainer's explicit OK:** Part A of the spec (the product itself) and anything that loosens the Guardian's defaults (section 11).
- **Check [docs/DECISIONS.md](docs/DECISIONS.md)** before reopening a choice that's already been made. It lists each key decision and the reason for it. New evidence is always welcome; the same argument again isn't.
- **Record what you decide.** If your change settles a design question, adds a notable dependency or borrows from another project, add a line to `docs/DECISIONS.md`, and keep the licence notices of any code you borrow.

## Setting up

You'll need:

- **Rust** through [rustup](https://rustup.rs). The repo pins the exact toolchain in `rust-toolchain.toml`, so rustup picks it up for you.
- **cargo-deny**, installed once with `cargo install --locked cargo-deny`.
- **A current Node.js LTS release** only if you're working on the desktop app's interface (from M7). Node is a build tool here; Wrybill never needs it at runtime.

Every change must pass these before review:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
```

CI runs the same four, then `cargo audit`, and builds and tests every target in section 14.1.

Tests never call live model APIs. Use the mock brain and recorded fixtures instead (section 18).

## Ground rules for code

These come from the spec and [AGENTS.md](AGENTS.md), and they apply to people and AI coding agents alike:

1. **Brains never touch the laptop.** Every action goes through the Guardian, front doors hold no business logic, and tools never call brains.
2. **Every Guardian rule ships with tests,** including red-team cases.
3. **Keep it light.** No LangChain-style frameworks, no Electron, and no Python or Node.js runtime inside Wrybill (non-goal N7).
4. **Be choosy with dependencies.** Prefer the standard library, then small, well-maintained crates with MIT or Apache-2.0 licences, and explain every new dependency in your pull request.
5. **Cross-platform by default.** No hard-coded paths or shells. Wrybill has to run on Intel Macs on macOS 11 and 12, Windows 10 and 11, and Linux.
6. **No `unsafe`** without a comment explaining why, and a test.
7. **No secrets or personal data** in code, tests, logs, fixtures or prompts. This repository is public.

## Using AI coding agents

You're welcome to use Claude Code, Codex CLI, Gemini CLI or any other coding agent. Wrybill itself is being built that way. The rules for agents are in [AGENTS.md](AGENTS.md), and most agents read that file automatically.

A few expectations:

- **You're responsible for every line you submit, including those produced with AI assistance.** Read it, run it, and be ready to explain it.
- **Keep secrets out of prompts.** Never paste API keys or private data into an agent session.
- **For Guardian changes,** carefully review the diff against the spec yourself, with the help of a coding agent that didn't write the change, before you open the pull request. The maintainer gives the second review.

## Pull requests

- Keep each pull request to one thing, with small commits and clear messages.
- Fill in the pull request template: what changed, which spec sections it touches, and how you tested it.
- Changes to the Guardian need red-team tests and two reviews before they merge: one by you, with the help of a coding agent that didn't write the change, and one by the maintainer.
- Never present a target from the spec as a measured result. If you measured something, include the numbers and the machine.

## Writing style

Docs, comments and messages that users see are written in plain, friendly English (summarise, behaviour, licence). Short sentences beat clever ones.

## Licensing your contribution

Wrybill is licensed under [Apache-2.0](LICENSE). Under section 5 of that licence, anything you intentionally submit for inclusion in Wrybill comes in under the same terms, unless you explicitly say otherwise. There's no contributor licence agreement to sign.

## Be kind, and report security issues privately

Everyone taking part follows our [Code of Conduct](CODE_OF_CONDUCT.md). If you find a security problem, please don't open a public issue. Follow [SECURITY.md](SECURITY.md) instead.

Ngā mihi, and thanks for helping build Wrybill.
