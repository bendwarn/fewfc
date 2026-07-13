# Agent Instructions

## Repository Validation

- This repository is a Rust crate. Prefer `cargo test` for the main validation path unless the task specifically touches package tooling.

## Online Game Commands

- Before changing pending command drafts, pending-choice payloads, or canonical
  events, read the command and replay constraints in `docs/rule.md` and
  `docs/rules-engine-decisions.md`.
- Rust Web DTO fields consumed by TypeScript must serialize with the exact
  camelCase contract. Add a serialization contract test for new multiword
  action fields; Rust field names otherwise default to snake_case and can turn
  a valid UI action into a server error.

## Agent Skills

### Issue tracker

Issues are tracked as local markdown files under `docs/local-issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary unless a local issue explicitly says otherwise. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context rules-engine repo; read `CONTEXT.md` and relevant decision docs before domain-sensitive work. See `docs/agents/domain.md`.
