# 39 Compose the Base Ruleset with official rule modules

## Triage

complete

## What to build

Deepen rule execution behind the mandatory Base Ruleset and closed official
Advanced Rule Module catalog recorded in ADR-0001. Preserve all Base Ruleset
behavior while routing setup, command decisions, automatic advancement,
playable-Formation queries, replay verification, and Web-facing execution
through the same interface.

Formation registries, matchers, Effect Plans, resolvers, and Pending Choice
continuations become implementation details. Game Setup and Game State carry the
enabled module configuration so callers cannot apply mismatched rules.

## Acceptance criteria

- [x] Every game executes the Base Ruleset whether or not Advanced Rule Modules are enabled.
- [x] Game State projects the enabled Advanced Rule Module identities from Game Setup.
- [x] Base setup, commands, automatic advancement, playable-Formation queries, and replay execute through one deep rule-execution interface.
- [x] Official Advanced Rule Modules augment that same command and query flow rather than replacing the Base Ruleset.
- [x] Game Record and Web callers do not construct rule modules or choose them independently of Game State.
- [x] Unknown Advanced Rule Module identities fail explicitly before emitting Game Events or mutating Game State.
- [x] Formation registry, matcher, and module-hook details are not part of the external interface.
- [x] Shared conformance tests exercise the Base-only configuration through the same interface used by callers.
- [x] Direct tests of shallow registry plumbing are replaced; matcher algorithm invariants may remain internal tests.
- [x] Existing Base-only Rust, Web unit, and browser behavior remains unchanged.

## Blocked by

None - can start immediately
