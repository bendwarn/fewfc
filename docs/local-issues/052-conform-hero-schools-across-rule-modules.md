# 52 Conform Hero Schools across Rule Modules

## Triage

ready-for-agent

## What to build

Complete Hero Schools 5.16 conformance across the Base Ruleset and every
available Rule Module. Enforce the accepted typed-hook and Attack-stage
decisions, close cross-School gaps, verify the full 18-Profession catalog, and
make direct execution, replay, Public Views, Web DTOs, and selected-Card queries
agree before the module is released.

## Acceptance criteria

- [ ] A conformance inventory maps every official Profession transition, inherited ability, Automatic Ability, Proficiency, Activated Ability, and Profession Formation to focused tests.
- [ ] Hero Schools and Star tests cover point-based summoning qualification, Star Element Substitution, Prepared interpretations, Sacred Art slots, and Five-Star Alignment ordering.
- [ ] Hero Schools and Five Directions Legend tests cover Environment damage, Formation ineffectiveness, Profession immunity, Counter Effect immunity, and Void resolution ordering.
- [ ] Hero Schools and Discard Retrieval tests cover Seeker cost modification, Cannot Act timing, lethal cost, and shared Team HP.
- [ ] Hero Schools and Personal Deck tests cover activation costs, Card Origin, Exposed Foreign Cards, per-Player piles, recycling, and reconnect.
- [ ] Each physical Card uses at most one element-and-level interpretation source per Formation Match Option; Sacred Art multiplicity composes only after that choice.
- [ ] Cross-module Attack resolution follows match, Attack Points, threshold qualification, Counter Effect, damage transformation and Shield, then post-Formation intents.
- [ ] Activated Profession Abilities remain usable under Cannot Act when their entire effect can resolve; preparations requiring a later Formation are rejected when no legal follow-up exists.
- [ ] Two-Player and four-Player tests cover Player-owned Professions, Team-owned HP, Previous Player targeting, and effects that address every other Player.
- [ ] Base-only, Hero-disabled, validation atomicity, record verification, viewer filtering, and existing room replay remain regression gates.
- [ ] The Hero Schools release gate remains closed.

## Blocked by

- [#47 Progress through the Seeker School](047-progress-through-the-seeker-school.md)
- [#48 Prepare Formations through the Mesmer School](048-prepare-formations-through-the-mesmer-school.md)
- [#49 Resolve the Mage School](049-resolve-the-mage-school.md)
- [#50 Resolve the Windwalker School](050-resolve-the-windwalker-school.md)
- [#51 Add Unaffiliated Professions and Void Reversion](051-add-unaffiliated-professions-and-void-reversion.md)
- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
- [#41 Use Star Formations and Star Element Substitution](041-use-star-formations-and-element-substitution.md)
- [#42 Resolve Void Star Breaking atomically](042-resolve-void-star-breaking-atomically.md)
- [#43 Win through Five-Star Alignment](043-win-through-five-star-alignment.md)
