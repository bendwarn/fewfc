# 48 Prepare Formations through the Mesmer School

## Triage

ready-for-agent

## What to build

Add the complete Mesmer School as an end-to-end Hero Schools slice. Implement
Mesmer, Spirit Mesmer, and Hermit with every official 5.16 transition,
inherited ability, Activated Profession Ability, Formation Proficiency,
Profession Formation, projection, and interaction from the
[Mesmer School rules](https://www.cfecards.org/rule/latest/hero/mesmer).

## Acceptance criteria

- [ ] All three Mesmer School Profession Changes enforce their official prerequisite and Water-level cost.
- [ ] Activated Profession Abilities use the shared once-per-Player-turn allowance and do not consume the action opportunity.
- [ ] Illusion and Phantasm create a serializable Prepared Profession Ability with declared Card, element, level, and Formation scope without mutating Card data.
- [ ] Prepared details are immediately public, survive reconnect and replay, and clear after any Action or at Turn End.
- [ ] Activation requires at least one possible legal follow-up, but the next Action is not forced to use the preparation and paid costs are never refunded.
- [ ] One physical Card selects at most one printed, Prepared, or Star-substituted interpretation in each Formation Match Option.
- [ ] Mesmer School Proficiencies and Profession Formations implement their official patterns and effects while retaining correct Formation identities.
- [ ] Element resistance, Seal interaction, Shield behavior, draw bonuses, and Profession replacement use typed hooks.
- [ ] Direct execution, replay, Public Views, Web DTOs, playable Actions, the Ability panel, and ability summaries agree.
- [ ] Tests cover disconnect after preparation, unused preparation, invalid activation, copied effects, Hero-disabled behavior, and team mode.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
