# 58 Compose Profession and Rule Module catalogs

## Triage

ready-for-agent

## What to build

Prepare the existing Rules Engine and Online Game Room configuration for
additional Theme Rule Modules without changing current game behavior. Compose
one Profession catalog from enabled module-owned definitions, support
multi-parent cross-module ability inheritance while retaining one Player-owned
Profession slot, and replace Spirit-specific dependency handling with one
closed Rule Module dependency graph. Remove the Profession teaching dialog while
retaining current Profession badges and effective-ability summaries.

## Acceptance criteria

- [x] Existing Hero Schools Profession Changes, inherited abilities, Profession Formations, replay, and Public Views remain behaviorally unchanged through the composed catalog.
- [x] Profession definitions support globally unique IDs, multiple explicit parents, stable ability deduplication, and transition predicates independent of inheritance.
- [x] Disabled Rule Modules contribute no Professions, Formations, abilities, or playable Actions.
- [x] Rule Module dependencies are declared in one closed catalog and normalized transitively by the Rust Rules Engine, server validation, shared room model, and waiting-room UI.
- [x] Every currently available Rule Module remains enabled by default for new games and rooms, while persisted rooms retain their stored module list.
- [x] The Profession teaching dialog and its trigger are removed; Player Profession badges and effective-ability summaries remain.
- [x] Focused Rust and Web tests cover catalog composition, dependency normalization, default selection, stored-room compatibility, and the removed teaching control.

## Blocked by

None - can start immediately
