# Agent Instructions

## Development Environment Tips

- This repository is a Rust crate. Prefer `cargo test` for the main validation path unless the task specifically touches package tooling.
- The Nuxt app lives under `apps/web`. Auth and online room APIs need Wrangler/Cloudflare bindings; plain `bun run dev` is only for Nuxt-only UI work.
- For `apps/web` Worker/Durable Object local dev, ensure `.dev.vars` exists and `BETTER_AUTH_SECRET` is at least 32 characters, otherwise Better Auth routes fail with `500 BETTER_AUTH_SECRET must contain at least 32 characters`.
- Wrangler must run under Node, not Bun. If `bun run cf:dev` fails with `Wrangler does not support the Bun runtime`, run `bun run build`, then start Wrangler with Node: `node node_modules/.bin/wrangler dev --env=""`. In Codex Desktop, use `load_workspace_dependencies` if `node` is not on `PATH`.
- Local dev servers may need sandbox escalation to bind localhost ports. If a dev server reports no available port while nothing is reachable, rerun with escalated permissions.

## Agent Skills

### Issue tracker

Issues are tracked as local markdown files under `docs/local-issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary unless a local issue explicitly says otherwise. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context rules-engine repo; read `CONTEXT.md` and relevant decision docs before domain-sensitive work. See `docs/agents/domain.md`.
