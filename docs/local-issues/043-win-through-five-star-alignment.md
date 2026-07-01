# 43 Win through Five-Star Alignment

## Triage

ready-for-agent

## What to build

Implement Five-Star Alignment as the Star Rule Module's Player-specific special
victory condition. When the module is enabled, a Player who has personally
summoned all five Star kinds wins for their Team after the qualifying Formation
Use fully resolves.

Represent the achievement as a semantic canonical Game Event and project it
through Game State, replay, Public State View, Public Event Feed, and the Web
game-result presentation.

## Acceptance criteria

- [x] Star summoning history is tracked per Player and retains distinct Star kinds after Stars are replaced or broken.
- [x] Teammates' histories are not combined when evaluating Five-Star Alignment.
- [x] Summoning a Player's fifth distinct Star emits `FiveStarAlignmentAchieved`.
- [x] The complete Formation Use resolves before Five-Star Alignment determines Game Outcome.
- [x] Five-Star Alignment makes the completing Player's Team the winner before HP-based outcome evaluation.
- [x] A simultaneous HP draw caused by the same Formation Use does not override Five-Star Alignment.
- [x] Public State View and Public Event Feed identify the special victory without exposing hidden canonical data.
- [x] The Web result presentation distinguishes Five-Star Alignment from an HP victory.
- [x] Direct execution and replay produce identical histories, achievement event, and Game Outcome.
- [x] Two-player and team-mode tests cover completion, repeated Star summons, and non-combining teammate histories.

## Blocked by

- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
