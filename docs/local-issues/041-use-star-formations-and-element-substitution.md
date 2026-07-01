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

- [x] Each Team-owned Star makes its official single-card and three-card Star Formations available to every Player on that Team.
- [x] Base-only games never match or expose Star Formations or Star Element Substitution.
- [x] The five single-card Star strikes are distinct elemental Attack Formations with fixed 10 Attack points.
- [x] The five three-card Star Formations use their official element patterns and level-sum-times-three Attack points.
- [x] An accepted three-card Star Formation grants one Turn Draw bonus and breaks its enabling Star.
- [x] Damage prevention does not cancel the three-card Star Formation's Turn Draw bonus or Star Breaking.
- [x] Star strikes and three-card Star Formations participate in normal elemental interaction, Shield, and Counter Effect resolution.
- [x] Star Formations are not Base Ruleset Formations and cannot be copied by class change.
- [x] Star Element Substitution implicitly considers at most one eligible Card Instance while matching a Base Ruleset Formation.
- [x] Star Element Substitution cannot match a Star Formation and does not mutate the Card Instance or Card Definition.
- [x] Playable-Formation queries and formation submission use identical matching rules.
- [x] The Web Formation flow presents and submits Star Formations without a Star-specific client command.
- [x] Canonical events, Public Event Feed entries, direct execution, and replay remain equivalent for every Star Formation and substitution path.

## Blocked by

- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
