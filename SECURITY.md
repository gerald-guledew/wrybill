# Security policy

Wrybill is being built to run commands, edit files and browse the web on people's own laptops. Security isn't something we'll bolt on later; it's the heart of the design (section 11 of [the spec](docs/SPEC.md)). Thanks for helping keep it safe.

## Project status

Wrybill is in early development and hasn't had a release yet. Until version 1.0, security fixes go into the latest code on the `main` branch and, once releases start, the latest release.

## Reporting a vulnerability

Please **don't** open a public issue, discussion or pull request for a security problem.

Report it privately through GitHub instead: open the repository's **Security** tab and choose **Report a vulnerability**. It helps to include:

- what you found and why it matters;
- steps or a proof of concept that shows it;
- the commit or version, your operating system, and the model you used, if it's relevant.

## What to expect

Wrybill has one maintainer for now, so these are goals rather than guarantees:

- an acknowledgement within 7 days;
- a first assessment within 14 days; and
- a fix or a plan agreed with you before anything is made public, with credit to you if you'd like it.

## What we most want to hear about

- Getting Wrybill to take an Ask-tier action without the user's approval, or any Deny-tier action at all.
- Escaping the sandbox, or writing outside the workspace when it shouldn't.
- Prompt injection: Wrybill acting on instructions from a web page, file, tool output, MCP server or helper.
- Private data reaching an outside destination without the user seeing it first (the taint rules in section 11.7).
- Wrybill making a connection that its network ledger doesn't record, or private mode sending anything to a cloud model or to a site the user hasn't approved.
- API keys or other secrets leaking into logs, transcripts or a model's context.
- Getting Wrybill, or anything it runs, to change the audit log without `wrybill log verify` noticing.
- An MCP server or skill changing after approval without Wrybill catching it.
- A downloaded project running code through its own config or hooks when Wrybill only meant to read it.

## Out of scope

- A model giving a wrong or unhelpful answer, when no unsafe action follows from it.
- Attacks that need an already compromised computer, or someone who already has the user's own access.
- Problems in model providers or other third-party services. Please report those to them.
- Social engineering of the maintainer or contributors.

## Researching in good faith

We welcome good-faith security research under this policy and won't take action against people who follow it. Please test only on machines and accounts you own, use test data, avoid harming other people's data or services, and give us a fair chance to fix things before you share details.
