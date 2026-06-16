# 33 Add deterministic deck preparation adapter

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 5.

## What to build

Add an adapter for preparing deck order deterministically before `DeckPrepared` is recorded. Replay must continue to depend only on the recorded deck order, while new game creation may optionally use a seeded or injected randomness source to produce that order.

## Acceptance criteria

- [ ] A deck preparation port can produce an ordered deck before game start.
- [ ] A deterministic in-memory or seeded adapter can produce repeatable deck orders for tests.
- [ ] `GameRecord::start` still records the full `DeckPrepared.deck_order`.
- [ ] Replay ignores seeds or RNG adapters and uses only recorded deck order.
- [ ] Tests cover deterministic preparation, event-log preservation of prepared order, and replay independence from RNG.

## Blocked by

- None - can start immediately

