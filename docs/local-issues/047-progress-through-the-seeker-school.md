# 47 Progress through the Seeker School

## Triage

ready-for-agent

## What to build

Add the complete Seeker School as an end-to-end Hero Schools slice. Implement
Seeker, Expounder, and Benevolent with their official 5.16 transitions,
inherited abilities, Formation Proficiencies, Profession Formations, combat
modifiers, Public View presentation, and Web Action candidates from the
[Seeker School rules](https://www.cfecards.org/rule/latest/hero/seeker).

## Acceptance criteria

- [x] All three Seeker School Profession Changes enforce their official prerequisite and Wood-level cost.
- [x] Seeker's Discard Retrieval modifier changes the HP cost only while the effective ability is owned.
- [x] Generating Formation, Overcoming Formation, Return to Origin, and Five Elements Cycle Proficiencies add alternative matchers without changing Formation identity.
- [x] Dao Defense resolves its official Attack and Counter Effect through the shared pipelines.
- [x] Benevolent's Wood resistance and Spell protection apply at typed resolution stages and disappear after Profession replacement or breaking.
- [x] Reincarnation implements its official pattern and recovery formula.
- [x] Reincarnation returns one Formation Match Option when only one assignment
  of its standalone Wood Card is legal; when both assignments are legal and
  produce different recovery values, it returns separate options with accurate
  previews and requires an explicit selection.
- [x] Public events record the selected role binding and resolved recovery without recomputing rules during replay.
- [x] Direct execution, replay, playable Actions, Web DTOs, and ability summaries agree.
- [x] Tests cover two-Player and team-mode targeting, Discard Retrieval enabled and disabled, Profession replacement, and Hero-disabled behavior.

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
