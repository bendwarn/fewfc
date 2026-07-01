# 46 Change Profession into the Warrior School

## Triage

ready-for-agent

## What to build

Deliver the first complete Hero Schools tracer through the Warrior School while
keeping the incomplete module behind a release gate. Add Player-owned
Profession state, the official Profession catalog and inheritance model,
Profession Change as a distinct Action Command, typed Profession Ability hooks,
canonical events, replay, Public Views, playable Actions, and the seat badge.

Implement Warrior, War God, and Hero with every official 5.16 ability,
Formation Proficiency, Profession Formation, transition requirement, and
inherited ability from the
[Warrior School rules](https://www.cfecards.org/rule/latest/hero/warrior).

## Acceptance criteria

- [x] Every Player starts without a Profession and Game State stores at most one `ProfessionId` per Player.
- [x] Profession Change validates the target Profession, prerequisite, and exact selected Card Instances as an Action Command rather than a Formation Use.
- [x] Accepted Profession Change enters the shared action-start pipeline, consumes the action opportunity, records explicit Card movement, and emits `ProfessionChanged`.
- [x] A new Profession replaces the previous Profession; effective abilities derive from catalog inheritance rather than copied state.
- [x] Typed hooks return declarative modifiers or intents and never mutate Game State or emit events directly.
- [x] Warrior, War God, and Hero implement all official transitions, inherited abilities, Proficiencies, and Profession Formations.
- [x] Profession Proficiencies retain the original Formation identity and effect.
- [x] Public State View, Public Event Feed, Web DTOs, recorded-event metadata, direct execution, and replay agree.
- [x] A Player with a Profession shows a public seat badge and read-only effective-ability summary; no badge appears without a Profession.
- [x] The selected-Card Action panel presents legal Profession Changes without enumerating other Card combinations.
- [x] The incomplete Hero Schools module cannot be enabled in production room configuration.
- [x] Two-Player and four-Player tests cover legal progression, illegal skipping, replacement, Profession Breaking, and Hero-disabled regression.

## Blocked by

- [#45 Deepen Card selection into playable Actions](045-deepen-card-selection-into-playable-actions.md)
