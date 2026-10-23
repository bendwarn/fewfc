# 78 Manage Durable Object Lifecycle with Wrangler Exports

## Triage

ready-for-agent

## Goal

Manage the web Worker Durable Object lifecycle declaratively with Wrangler
`exports`, using SQLite for new namespaces and preserving the existing
GameRoom, PlayerNotifications, ReplayArchive, and CompetitionRoom namespaces.

## Confirmed Decisions

- Keep all four class names, bindings, and existing SQLite storage backends.
  The default, staging, and production configurations created these classes
  through `new_sqlite_classes`; this work does not rename, delete, transfer, or
  re-provision a namespace.
- Replace the Durable Object `migrations` history in each Worker environment
  with a current-state `exports` declaration for all four classes. Durable
  Object `exports` and `migrations` cannot coexist, and after the first exports
  deployment the Worker must stay on the exports lifecycle.
- Keep D1 `migrations_dir` settings and SQL migration files unchanged.
- Preserve the local legacy-game cutover wizard's maintenance switch. Manage
  the `MAINTENANCE_MODE` Worker variable on the deployed environment and apply
  each value immediately through configuration updates compatible with Durable
  Object exports. The wizard proceeds to purge only after the maintenance
  update succeeds.
- Apply Durable Object lifecycle changes only through `wrangler deploy`; do not
  use gradual deployment for lifecycle changes. This implementation changes
  local configuration and docs only; it does not deploy to Cloudflare.

## Plan

1. Confirm each live class and storage backend from the Worker code and the
   original Wrangler migration entries.
2. Declare the current class set under `exports` for default, staging, and
   production, then remove only the Durable Object migration entries.
3. Update the cutover maintenance switch and deployment guide to manage
   `MAINTENANCE_MODE` through Worker variable configuration compatible with
   Durable Object exports.
4. Record the issue complete after reviewing the configuration and diff; leave
   the first remote deployment for the normal protected deploy workflow.

## Acceptance Criteria

- [x] Each Worker environment declares all four current Durable Object classes
  with `storage = "sqlite"` under `exports`.
- [x] The Wrangler file contains no Durable Object `migrations` array; D1
  database migration configuration remains intact.
- [x] The legacy-game cutover wizard sets maintenance mode as a Worker variable
  through configuration updates compatible with Durable Object exports.
- [x] Deployment documentation explains the exports lifecycle and maintenance
  switch behavior.
- [x] No Durable Object runtime code, schema, stored data, or namespace identity
  changes, and no remote deployment is performed.

## Implementation And Verification

Updated the default, staging, and production Wrangler configuration to declare
the existing four SQLite-backed namespaces through `exports`. Kept the D1
migration directories intact. Updated the local cutover wizard to set
`MAINTENANCE_MODE` as a Worker variable and revised the deployment guide to
describe the one-way exports switch and deploy constraints.

Reviewed the config declarations and `git diff --check` passed. Tests and remote
deployment were not run.
