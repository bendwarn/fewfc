# 53 Release Hero Schools to Online Rooms

## Triage

ready-for-agent

## What to build

Release the complete Hero Schools Rule Module to Online Game Rooms as one
default-on Advanced Rule toggle. Preserve existing room configurations, expose
all 18 Professions and their public data, finish the vertically stacked Ability
and Action controls, add teaching-mode Profession progression, and validate
complete two-Player and four-Player online games.

## Acceptance criteria

- [x] The Base Ruleset remains mandatory and room setup exposes one Hero Schools toggle with no per-School or per-Profession controls.
- [x] New rooms enable Hero Schools by default; existing waiting rooms and active games retain their stored module list and remain Hero-disabled.
- [x] Enabling Hero Schools in an existing waiting room invalidates readiness and locked setup inputs through the existing Rule Module change behavior.
- [x] Room metadata, lists, invitations, start, reconnect, reset match, recorded decisions, replay, and Public Views preserve the Hero setting.
- [x] The release gate opens only after the complete 18-Profession catalog and all conformance tests are present.
- [x] The upper Ability panel lists legal Active-Effect Commands and explains that they do not end the turn action.
- [x] The lower Action panel lists legal Formation Uses, Profession Changes, and Pass Action from the selected Cards and explains that they end the Main Phase.
- [x] Both panels remain vertically ordered at every viewport width with actionable controls, focus behavior, and complete accessible names.
- [x] Profession badges appear only for Players with a Profession and open a read-only effective-ability summary.
- [x] Teaching mode presents the complete progression graph, transition requirements, ability inheritance and loss, and Profession Formation summaries without changing game legality.
- [x] Prepared Profession Ability details, Profession Changes, Profession Breaking, and Void Reversion results appear in public online history and survive reconnect.
- [x] Self-contained Playwright flows cover default-on and disabled Hero settings, two-Player and four-Player games, at least one Profession progression and activation, reconnect, room reset, and every available Rule Module enabled together.
- [x] Existing Base-only and Optional Rule browser flows remain green.

## Blocked by

- [#52 Conform Hero Schools across Rule Modules](052-conform-hero-schools-across-rule-modules.md)
- [#44 Toggle Star advanced rules in Online Game Rooms](044-toggle-star-rules-in-online-game-rooms.md)
