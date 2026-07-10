---
status: accepted
---

# Separate action detail from catalog rule text

Web playable-action detail is a player-facing decision aid, not a direct dump of
`FormationDef.rule_text`.

The Formation Catalog keeps concise canonical formation text for matching,
catalog display, and engine identity. A playable action may need richer text
because pressing that action can create follow-up choices, trusted randomness,
delayed schedules, or module-specific exceptions that are not part of the
formation's immediate main effect.

Echo exposes the boundary. Each Melody has a main effect, but the player-facing
detail also needs to state the Echo policy:

- 商調‧鳴金, 角調‧落木, 羽調‧流水, 徵調‧戰火, and 宮調‧裂土 may schedule Echo
  only after the complete main effect resolves and the Player discards one Card
  with an allowed printed element.
- 變徵‧淨火 schedules Echo automatically and pays no Echo Cost.
- 變宮‧植土 schedules its own next-Turn-Start choice and never schedules Echo.
- Echo and 植土 later execute only the Melody main effect; they are not new
  Formation Uses.

Therefore Web action-detail generation should be rule-aware. It can start from
catalog rule text, but must append or compose the visible consequences for the
specific action being offered. Tests should assert the serialized
`PlayableAction.summary` contract for Echo rather than relying on tooltip
screenshots.
