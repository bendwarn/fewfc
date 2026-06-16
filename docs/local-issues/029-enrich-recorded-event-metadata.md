# 29 Enrich recorded event metadata

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 34 and closed GitHub issue #4.

## What to build

Expand recorded event metadata so event feeds can identify setup, automatic, and command-produced events with enough information for debugging, UI acknowledgement, networking, and audit. Replay should continue to depend only on event sequence and canonical event payloads.

## Acceptance criteria

- [ ] `RecordedEvent` metadata retains monotonic sequence numbers.
- [ ] Event source distinguishes setup, automatic, and command events.
- [ ] Automatic event metadata includes a deterministic reason where useful.
- [ ] Command event metadata can identify the command/player context without persisting a separate command log.
- [ ] Replay uses only the canonical `GameEvent` sequence, not command metadata.
- [ ] Tests cover metadata source classification for setup events, automatic events, command events, and replay independence from metadata.

## Blocked by

- None - can start immediately

