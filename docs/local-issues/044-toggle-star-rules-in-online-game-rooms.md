# 44 Toggle Star advanced rules in Online Game Rooms

## Triage

ready-for-agent

## What to build

Every Online Game Room uses the Base Ruleset. Add one default-on room setting
that enables the complete Star Rule Module; creators may turn off that module
without disabling any Base rule.

Persist the enabled module configuration as immutable room metadata and use it
for setup, command handling, automatic advancement, playable-Formation queries,
Public View updates, room lists, reconnects, and replay.

## Acceptance criteria

- [ ] Base Ruleset is mandatory for every room and the UI provides no control to disable it.
- [ ] Room creation provides one Star advanced-rules toggle that is enabled by default.
- [ ] Disabling the toggle leaves every Base rule active.
- [ ] The UI does not expose separate toggles for individual Stars or Star features.
- [ ] The enabled module configuration is immutable after room creation.
- [ ] Public and private room metadata, room lists, invitations, reconnects, and reset matches retain the Star toggle.
- [ ] Starting a room creates Game Setup with the mandatory Base Ruleset and the selected module configuration.
- [ ] Every later command, automatic advancement, playable-Formation query, and replay dispatches from the Game State module configuration.
- [ ] Public State View and Public Event Feed carry complete viewer-safe Star data only when the Star Rule Module is enabled.
- [ ] Rooms with Star disabled retain Base behavior and do not expose Star state or Star Formations.
- [ ] Room creation and waiting-room UI clearly show that Base is mandatory and whether Star rules are enabled.
- [ ] Browser tests cover default-enabled Star rules, explicitly disabled Star rules, two-player and team-mode starts, reconnect, and match reset.
- [ ] A complete match with Star rules enabled can be played online without debug controls or manual phase advancement.

## Blocked by

- [#41 Use Star Formations and Star Element Substitution](041-use-star-formations-and-element-substitution.md)
- [#42 Resolve Void Star Breaking atomically](042-resolve-void-star-breaking-atomically.md)
- [#43 Win through Five-Star Alignment](043-win-through-five-star-alignment.md)
