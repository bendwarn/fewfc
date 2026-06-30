---
status: accepted
---

# Resolve Profession Abilities through typed rule hooks

Profession Abilities integrate through explicit, typed Hero Schools module
hooks at the rule stages they modify, including Formation matching, costs,
Attack calculation, immunity, Counter Effects, and post-Formation consequences.
Hooks return declarative modifiers or intents for the shared pipeline to record
and apply; they do not mutate Game State or emit Game Events directly.

Generic Status Effect entries were rejected because a Profession Ability exists
by owning its Profession rather than by applying an ongoing effect. Centralized
Profession-specific conditionals were rejected because they couple the Base
Ruleset to one optional module and make cross-module ordering implicit.
