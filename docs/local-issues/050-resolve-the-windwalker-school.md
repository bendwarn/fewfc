# 50 Resolve the Windwalker School

## Triage

ready-for-agent

## What to build

Add the complete Windwalker School as an end-to-end Hero Schools slice.
Implement Windwalker, Shadow Walker, and Martial Artist with every official
5.16 transition, inherited ability, Activated Profession Ability, Formation
Proficiency, Profession Formation, immunity, projection, and Web interaction
from the
[Windwalker School rules](https://www.cfecards.org/rule/latest/hero/windwalker).

## Acceptance criteria

- [x] All three Windwalker School Profession Changes enforce their official prerequisite and Earth-level cost.
- [x] Windwalking ignores Formation Counter Effects only for Attacks whose computed Attack Points are 15 or lower.
- [x] Environment and elemental damage changes do not alter the Windwalking threshold decision.
- [x] Shadow Cut discards the selected Card, directly changes the Previous Player's Team HP by twice its level, consumes the shared activation allowance, and leaves the Action opportunity open.
- [x] Metamorphosis and Chaos Proficiencies retain the original Formation identities and effects.
- [x] Shadow Assault is a Spell that directly changes HP, bypasses Shield and Attack Counter Effects, and remains subject to Seal.
- [x] Earth resistance and Shadow Escape apply only to their officially named damage and Formation effects.
- [x] Instant Shadow Death is recorded as an HP deduction, sets the Previous
  Player's Team HP to `floor(original HP / 2)`, and does not route through
  Shield damage.
- [x] Direct execution, replay, Public Views, Web DTOs, playable Actions, the Ability panel, and ability summaries agree.
- [x] Tests cover lethal Shadow Cut before an Action, copied effects, Environment interaction, team HP, and Hero-disabled behavior.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
