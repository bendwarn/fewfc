# 25 Generalize effect intents for spell resolvers

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 25 and closed GitHub issue #18.

## What to build

Deepen the effect resolver boundary so attack, immediate spell, and passive spell resolvers return declarative intents that the formation pipeline converts into semantic events. This should support additional consequences such as HP changes, shield changes, statuses, card moves, action modifications, and pending choices without letting resolvers mutate `GameState` or emit events directly.

## Acceptance criteria

- [ ] Effect intent model supports shield changes, status changes, card moves, HP changes, action modifications, and pending choices.
- [ ] Existing `barrier` and `metamorphosis` behavior is expressed through the generalized intent path.
- [ ] The formation pipeline owns baseline used-card movement.
- [ ] Effect resolvers remain deterministic and do not mutate state or append events directly.
- [ ] The pipeline rejects duplicate pending choices before emitting events.
- [ ] Tests cover at least one shield intent, one status intent, one pending-choice intent, and one action-modification intent through public command APIs.

## Blocked by

- None - can start immediately

