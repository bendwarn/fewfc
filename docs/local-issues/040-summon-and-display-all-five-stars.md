# 40 Summon and display all five Stars

## Triage

ready-for-agent

## What to build

Add the Star Rule Module to the closed official module catalog and implement
Star Summoning for 金星‧太白, 木星‧歲星, 水星‧辰星, 火星‧熒惑, and
土星‧鎮星. When the module is enabled, a qualifying use of 鍠金, 樸木, 洄水,
熾火, or 坱土 grants the associated Star to the attacking Player's Team,
records the Player's personal summoning history, and applies replacement and
opposing-Star breaking rules.

Project the resulting canonical Star data through replay, Public State View,
Public Event Feed, Web DTOs, and the game display. Follow the official
[Star advanced rules](https://www.cfecards.org/rule/latest/star) and the terminology in
`CONTEXT.md`.

## Acceptance criteria

- [x] The official module catalog enables the Star Rule Module alongside the mandatory Base Ruleset.
- [x] Base-only games never summon or expose Stars.
- [x] Each of the five named Base Formations summons its associated Star when its Attack points are at least 30.
- [x] Summoning qualification uses Attack points before elemental interaction, Counter Effects, Shields, or HP resolution.
- [x] Star Formations, copied effects, and other three-card attacks cannot summon a Star.
- [x] A Team owns at most one Star; a newly summoned Star replaces that Team's previous Star.
- [x] A Star can be owned by at most one Team, and an already-owned Star is not summoned again or added again to Player history.
- [x] Each Star summons with the official opposing-Star breaking relationship.
- [x] `StarSummoned` and `StarBroken` are semantic canonical Game Events rather than Status Effect strings.
- [x] Game State retains current Team Stars and distinct Star kinds summoned by each Player.
- [x] Public State View and Public Event Feed expose Star ownership and summoning without exposing canonical replay data directly.
- [x] The Web game display shows current Team Stars and viewer-safe summoning progress.
- [x] Direct execution and replay produce identical Star state in two-player and team-mode games.

## Blocked by

- [#39 Compose the Base Ruleset with official rule modules](039-compose-base-ruleset-with-official-rule-modules.md)
