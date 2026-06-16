# 27 Complete rule-derived target resolution

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` sections 19 and 20, and closed GitHub issue #19.

## What to build

Add a reusable target resolver for rule-derived semantic targets. The engine already derives previous player for attack base damage; this slice should complete the same pattern for self, next player, own side, and opposing side so effect resolvers do not require player-submitted targets for deterministic rule-defined targets.

## Acceptance criteria

- [ ] Engine exposes internal target resolution for self, previous player, next player, own side, and opposing side.
- [ ] Rule-derived targets are resolved from current state and turn order, not from `declared_targets`.
- [ ] `declared_targets` remain reserved for concrete player/team/card choices required by effects.
- [ ] Invalid declared targets fail validation without events or state changes.
- [ ] Tests cover two-player and team-mode target resolution for all supported semantic targets.
- [ ] Tests cover at least one effect or attack path using a non-previous-player derived target.

## Blocked by

- #25

