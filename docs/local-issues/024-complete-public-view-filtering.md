# 24 Complete public state and event filtering

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` sections 7 and 25, and closed GitHub issue #21.

## What to build

Expand viewer-filtered public state and event views from covered passives and pending choices into a complete boundary suitable for external consumers. Canonical state and events must remain complete for replay, while public views must not leak hidden hands, initial deal cards, draw choices, covered cards, or effect-choice options to unauthorized viewers.

## Acceptance criteria

- [ ] Public state view includes the key game state needed by clients: phase, current player, turn number, teams/HP, hand sizes or own hand, discard, covered passives, pending choice, shields, statuses, and game status.
- [ ] A player can see their own hand and hidden choice/card options where authorized.
- [ ] Other players and observers see hidden-card counts rather than hidden card ids.
- [ ] Event views filter `CardsDealt`, turn draw choices, covered passive events, and effect-choice events consistently.
- [ ] Canonical `GameState`, `GameEvent`, and `GameRecord` remain unchanged as replay sources.
- [ ] Tests cover owner view, opponent view, observer view, initial deal filtering, turn draw filtering, covered passive filtering, effect choice filtering, and canonical replay preservation.

## Blocked by

- None - can start immediately

