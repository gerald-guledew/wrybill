## What this changes

<!-- One or two sentences. Link the issue or the milestone task it belongs to. -->

## Spec sections touched

<!-- For example: 11.5 How commands are classified. If the spec itself needs to change, say so here. -->

## How it was tested

<!-- What you ran, what you didn't, and why. Include numbers and the machine if you measured anything. -->

## New dependencies

<!-- Each new crate or package, and why it's worth it. Write "None" if there aren't any. -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo deny check` pass
- [ ] No secrets, personal data or machine-specific paths
- [ ] Any Guardian change has red-team tests, my own review with the help of a coding agent that didn't write it, and the maintainer's review (or this doesn't touch the Guardian)
- [ ] Docs and user-facing text use English
