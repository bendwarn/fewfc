# 41 Use Star Formations and Star Element Substitution

## Triage

ready-for-agent

## What to build

Implement all ten Star Formations and Star Element Substitution through the deep
rule-execution interface when the Star Rule Module is enabled. Players whose
Team owns a Star can query and perform its single-card Star strike and
three-card Star Formation. The owned Star may also implicitly substitute one
matching generating-element Card Instance while matching a Base Ruleset
Formation.

Keep Star Formation availability, validation, effect resolution, query results,
Public Event Feed presentation, and replay behind the same Ruleset interface.

## Acceptance criteria

- [ ] Each Team-owned Star makes its official single-card and three-card Star Formations available to every Player on that Team.
- [ ] Base-only games never match or expose Star Formations or Star Element Substitution.
- [ ] The five single-card Star strikes are distinct elemental Attack Formations with fixed 10 Attack points.
- [ ] The five three-card Star Formations use their official element patterns and level-sum-times-three Attack points.
- [ ] An accepted three-card Star Formation grants one Turn Draw bonus and breaks its enabling Star.
- [ ] Damage prevention does not cancel the three-card Star Formation's Turn Draw bonus or Star Breaking.
- [ ] Star strikes and three-card Star Formations participate in normal elemental interaction, Shield, and Counter Effect resolution.
- [ ] Star Formations are not Base Ruleset Formations and cannot be copied by class change.
- [ ] Star Element Substitution implicitly considers at most one eligible Card Instance while matching a Base Ruleset Formation.
- [ ] Star Element Substitution cannot match a Star Formation and does not mutate the Card Instance or Card Definition.
- [ ] Playable-Formation queries and formation submission use identical matching rules.
- [ ] The Web Formation flow presents and submits Star Formations without a Star-specific client command.
- [ ] Canonical events, Public Event Feed entries, direct execution, and replay remain equivalent for every Star Formation and substitution path.

## Blocked by

- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
