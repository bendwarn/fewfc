# Agent Instructions

## Repository Validation

- This repository is a Rust crate. Prefer `cargo test` for the main validation path unless the task specifically touches package tooling.
- For Web UI work, read the Storybook visual acceptance and test-layer rules in
  [Web App Agent Instructions](apps/web/AGENTS.md#ui-browser-and-e2e-validation).

## Online Game Commands

- Before changing pending command drafts, pending-choice payloads, or canonical
  events, read the command and replay constraints in `docs/rule.md` and
  `docs/rules-engine-decisions.md`.
- Rust Web DTO fields consumed by TypeScript must serialize with the exact
  camelCase contract. Add a serialization contract test for new multiword
  action fields; Rust field names otherwise default to snake_case and can turn
  a valid UI action into a server error.

## Operational terminology

- **GitHub Workflow (GHW)**: a remote automation defined in
  `.github/workflows/*.yml` and run by the GitHub Actions platform.
- **Toolbar Action (TA)**: a user-started command defined in
  `.codex/environments/environment.toml` and run in Codex's local integrated
  terminal.
- Do not use `GA`: it can mean the GitHub Actions platform or an individual
  `uses:` action. Outside concise comparison tables, spell out `GitHub
  Workflow` and `Toolbar Action`.

## 雜項
- ./scratch 可以放不用加到repo的檔案，避免需要在repo建立檔案
- 程式註解使用中文

### Issue tracker

Issues are tracked as local markdown files under `docs/local-issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary unless a local issue explicitly says otherwise. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context rules-engine repo; read `CONTEXT.md` and relevant decision docs before domain-sensitive work. See `docs/agents/domain.md`.
