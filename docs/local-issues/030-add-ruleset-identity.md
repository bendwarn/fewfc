# 30 Add ruleset identity to setup and persistence

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` sections 4, 27, and 31, and closed GitHub issue #22.

## What to build

Make ruleset identity an explicit part of game setup and persisted records. Persistence metadata currently carries ruleset/version information, but the authoritative setup should also identify which ruleset was used to validate setup, resolve formations, and replay records.

## Acceptance criteria

- [ ] `GameSetup` includes ruleset identity.
- [ ] Official setup builders populate the base ruleset identity.
- [ ] Persisted event logs include ruleset identity consistently with setup metadata.
- [ ] Loading/replay validates that persisted metadata and setup ruleset identity agree.
- [ ] Tests cover base setup builder identity, persistence round trip, and mismatch rejection.

## Blocked by

- None - can start immediately

