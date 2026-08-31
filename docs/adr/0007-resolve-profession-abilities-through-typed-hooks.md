---
status: accepted
---

# Resolve Profession Abilities through typed rule hooks

Profession Abilities integrate through explicit, typed Hero Schools module
hooks at the rule stages they modify, including Formation matching, costs,
Attack calculation, immunity, Counter Effects, and post-Formation consequences.
Hooks return declarative modifiers or intents for the shared pipeline to record
and apply; they do not mutate Game State or emit Game Events directly.

Activated Profession Abilities use a separate typed provider contract because
they begin a Player-selected Active Effect rather than modifying an existing
rule stage. Hero Schools, Jianghu, Confluence Generation, and Dark Glimmer each
provide a closed ability enum plus offer and effect plans. The shared Profession
module owns ability availability, the once-per-turn allowance, Action Card
Selection validation, exact completion input, player-facing detail, and the
initial canonical event batch. It records exactly one
`ProfessionAbilityActivated` event first, followed by provider consequences and
at most one final typed Choice or Randomness continuation.

Providers may build consequences against a scratch projection so a later rule
step observes earlier costs. They still do not mutate the authoritative Game
State. The provider contract is statically dispatched and exhaustively matched;
it is not a runtime registry or a plug-in seam.

Generic Status Effect entries were rejected because a Profession Ability exists
by owning its Profession rather than by applying an ongoing effect. Centralized
Profession-specific conditionals were rejected because they couple the Base
Ruleset to one optional module and make cross-module ordering implicit.
Per-ability runtime registration was rejected because the official Ruleset is a
closed composition and compile-time exhaustive dispatch detects missing ability
handling more reliably.
