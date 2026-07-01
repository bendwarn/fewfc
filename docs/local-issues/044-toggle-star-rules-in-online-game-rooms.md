# 44 Toggle Star advanced rules in Online Game Rooms

## Triage

ready-for-agent

## What to build

Every Online Game Room uses the Base Ruleset. Add one default-on room setting
that enables the complete Star Rule Module; creators may turn off that module
without disabling any Base rule.

Persist the enabled module configuration as room metadata. The owner may change
it while the room is waiting; every change invalidates non-owner readiness and
locked setup inputs. Starting the game captures an immutable module
configuration in Game Setup for command handling, automatic advancement,
playable-Formation queries, Public View updates, reconnects, and replay.

## Acceptance criteria

- [x] Base Ruleset is mandatory for every room and the UI provides no control to disable it.
- [x] Room creation enables the complete Star advanced-rules module by default; its waiting-room toggle is enabled by default.
- [x] Disabling the toggle leaves every Base rule active.
- [x] The UI does not expose separate toggles for individual Stars or Star features.
- [x] Only the owner can change enabled Rule Modules while the room is waiting.
- [x] Changing enabled Rule Modules invalidates non-owner readiness and all waiting-room locked setup inputs.
- [x] A started Game Setup retains an immutable module configuration and active-game commands cannot change it.
- [x] Public and private room metadata, room lists, invitations, reconnects, and reset matches retain the Star toggle.
- [x] Starting a room creates Game Setup with the mandatory Base Ruleset and the selected module configuration.
- [x] Every later command, automatic advancement, playable-Formation query, and replay dispatches from the Game State module configuration.
- [x] Public State View and Public Event Feed carry complete viewer-safe Star data only when the Star Rule Module is enabled.
- [x] Rooms with Star disabled retain Base behavior and do not expose Star state or Star Formations.
- [x] The waiting-room UI shows that Base is mandatory and whether Star rules are enabled; the creation dialog intentionally omits rule controls.
- [x] Browser tests cover default-enabled Star rules, explicitly disabled Star rules, waiting-room changes and readiness invalidation, two-player and team-mode starts, reconnect, and match reset.
- [x] A development-only Star endgame fixture is completed through normal online UI play, outcome resolution, and match reset without manual phase advancement.

## Blocked by

- [#41 Use Star Formations and Star Element Substitution](041-use-star-formations-and-element-substitution.md)
- [#42 Resolve Void Star Breaking atomically](042-resolve-void-star-breaking-atomically.md)
- [#43 Win through Five-Star Alignment](043-win-through-five-star-alignment.md)
