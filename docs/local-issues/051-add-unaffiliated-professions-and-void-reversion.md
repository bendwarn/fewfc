# 51 Add Unaffiliated Professions and Void Reversion

## Triage

ready-for-agent

## What to build

Add First Wanderer, Immortal, and Saint with every official 5.16 transition,
ability, Profession Formation, and non-inheritance rule from the
[Unaffiliated Profession rules](https://www.cfecards.org/rule/latest/hero/others).
Implement Sacred Art's physical-Card versus match-slot semantics and complete
Void Reversion Technique as the Hero Schools-wide Profession Breaking
Formation.

## Acceptance criteria

- [x] First Wanderer is acquired through Profession Change rather than assigned at setup.
- [x] Unaffiliated Professions never count as one School, including two Players who hold the same Unaffiliated Profession.
- [x] Choice and Breakthrough are legal Profession Change routes and do not consume the Activated Profession Ability allowance.
- [x] Immortal and Saint can be reached only from their official prerequisite Professions and do not retain the replaced Profession's abilities.
- [x] Every Unaffiliated Profession ability and Profession Formation implements its official pattern, formula, target, and effect.
- [x] Sacred Art may expand one physical Card into two match slots while Card movement and ordinary level-sum formulas count it once.
- [x] The Web Formation Match Option marks a Sacred Art Card as `×2` while showing physical discard count and point preview.
- [x] Void Reversion Technique matches three same-level Cards only while Hero Schools is enabled.
- [x] `VoidReversionResolved` atomically records the performing Team's -20 HP, Card movement, every broken Profession, and every retained low-level-protected Legendary Profession.
- [x] Level-one and level-two Void Reversion retains Legendary Professions; level-three-or-higher Void Reversion breaks them.
- [x] All HP and Profession deltas apply before Game Outcome evaluation, including when the cost defeats the performing Team.
- [x] Direct execution, replay, Public Views, Web DTOs, playable Actions, and ability summaries agree.
- [x] Tests cover every transition source, shared Team HP, simultaneous outcomes, no-Profession targets, and Hero-disabled behavior.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
