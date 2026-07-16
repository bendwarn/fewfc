# 48 Prepare Formations through the Mesmer School

> **Superseded in part:**
> [ADR 0025](../adr/0025-model-illusion-cards-as-virtual-formation-components.md)
> replaces this issue's Illusion and Phantasm preparation model. The completed
> checklist below remains a historical record, but its third-physical-Card,
> optional-follow-up, and mutually exclusive interpretation-source criteria are
> no longer authoritative. Illusion and Phantasm now create a Virtual Formation
> Card and a mandatory same-turn Formation Requirement.

## Triage

ready-for-agent

## What to build

Add the complete Mesmer School as an end-to-end Hero Schools slice. Implement
Mesmer, Spirit Mesmer, and Hermit with every official 5.16 transition,
inherited ability, Activated Profession Ability, Formation Proficiency,
Profession Formation, projection, and interaction from the
[Mesmer School rules](https://www.cfecards.org/rule/latest/hero/mesmer).

## Acceptance criteria

- [x] All three Mesmer School Profession Changes enforce their official prerequisite and Water-level cost.
- [x] Activated Profession Abilities use the shared once-per-Player-turn allowance and do not consume the action opportunity.
- [x] Illusion and Phantasm create a serializable Prepared Profession Ability with declared Card, element, level, and Formation scope without mutating Card data.
- [x] Phantasm does not trigger Illusion Refinement because they are separate Activated Profession Abilities.
- [x] Prepared details are immediately public, survive reconnect and replay, and clear after any Action or at Turn End.
- [x] Activation requires at least one possible legal follow-up, but the next Action is not forced to use the preparation and paid costs are never refunded.
- [x] One physical Card selects at most one printed, Prepared, or Star-substituted interpretation in each Formation Match Option.
- [x] Mesmer School Proficiencies and Profession Formations implement their official patterns and effects while retaining correct Formation identities.
- [x] Element resistance, Seal interaction, Shield behavior, draw bonuses, and Profession replacement use typed hooks.
- [x] Direct execution, replay, Public Views, Web DTOs, playable Actions, the Ability panel, and ability summaries agree.
- [x] Tests cover disconnect after preparation, unused preparation, invalid activation, copied effects, Hero-disabled behavior, and team mode.

## Superseded acceptance criteria

ADR 0025 supersedes the criteria that model Illusion or Phantasm as a
`PreparedProfessionAbility` targeting a physical Card Instance, allow the
Player to end the action without using the created Card, or choose exactly one
of printed, Prepared, and Star interpretations. The remaining Mesmer School
delivery criteria continue to describe the historical scope of this completed
issue.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
