# 26 Implement remaining base active spell effects

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` sections 25 and 31, and closed GitHub issues #11 and #18.

## What to build

Implement the remaining base active spell effects that currently return `EffectNotImplemented`. Each spell should resolve through the normal `PerformFormation` pipeline, consume the action only after validation succeeds, produce replayable semantic events, and use effect intents rather than mutating state directly.

## Acceptance criteria

- [ ] `generating-formation` has a deterministic first-version effect.
- [ ] `overcoming-formation` has a deterministic first-version effect.
- [ ] `radiance` has a deterministic first-version effect.
- [ ] `return-to-origin` has a deterministic first-version effect.
- [ ] `chaos` has a deterministic first-version effect.
- [ ] `five-elements-cycle` has a deterministic first-version effect.
- [ ] Known active spell behavior is covered by command-level integration tests and replay consistency tests.
- [ ] Legal but still intentionally deferred effects remain represented by explicit implementation errors, if any remain.

## Blocked by

- #25

