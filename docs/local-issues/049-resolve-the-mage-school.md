# 49 Resolve the Mage School

## Triage

ready-for-agent

## What to build

Add the complete Mage School as an end-to-end Hero Schools slice. Implement
Mage, Mage Guide, and Sage with every official 5.16 transition, inherited
ability, Formation Proficiency, Profession Formation, point modifier, Counter
Effect, projection, and Web interaction from the
[Mage School rules](https://www.cfecards.org/rule/latest/hero/mage).

## Acceptance criteria

- [x] All three Mage School Profession Changes enforce their official prerequisite and Fire-level cost.
- [x] Every official Automatic Profession Ability and Formation Proficiency is available only through the effective Mage School Profession.
- [x] Every Mage Profession Formation uses its official pattern, category, point formula, target, and additional effect.
- [x] Profession point modifiers apply while computing Attack Points before threshold qualifications and damage transformations.
- [x] Mage Counter Effects use the shared action-start and modifier pipelines rather than a parallel trigger system.
- [x] Attack events retain Attack Points separately from Counter Effect, elemental, Environment, Shield, and final HP results.
- [x] Replacing or breaking the Profession immediately removes automatic modifiers while preserving already-established public Counter Effects according to their own lifetime.
- [x] Direct execution, replay, Public Views, Web DTOs, playable Actions, and ability summaries agree.
- [x] Tests cover Metamorphosis copies, Shield targets, prevented damage with remaining extra effects, team mode, and Hero-disabled behavior.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
