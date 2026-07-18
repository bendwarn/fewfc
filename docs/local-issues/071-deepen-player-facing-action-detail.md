# 71 Deepen Player-Facing Action Detail

## Triage

ready-for-agent

## Problem Statement

The explanation shown before a Player commits an Action is currently assembled
from several shallow and overlapping sources. Rules Engine action candidates
mostly copy Formation Catalog `rule_text` into a generic `summary`, while the
Web appends Formation-ID policies, Star substitution prose, and separate Secret
Strategy text. Discard Retrieval has another dedicated presentation path.

As a result, the Rules Engine can change a cost, follow-up choice, trusted
random result, delayed effect, or Rule Module exception without forcing the
Player-facing explanation to change. Catalog prose can appear complete while
omitting consequences that matter to the current decision. Echo and Chain make
this drift especially visible because their main effects, optional costs,
delayed schedules, and nested choices cross several rule lifecycles.

## Solution

Represent player-visible rule consequences as a closed typed Rules Engine
contract and compose them into a required, Player-scoped, state-specific
`PlayerFacingActionDetail` embedded in each pre-commit Action offer. Rule
behavior supplies reusable consequence descriptors next to execution logic; a
central Player-Facing Action Detail module assembles and checks the complete
snapshot. The Web exhaustively renders those facts as localized player-facing
content without selecting rule prose by Formation or Secret Strategy identity.

Keep Formation Catalog prose, Pending Choice lifecycle data, canonical Game
Events, command payloads, and final-state simulation separate. Pending Choice
descriptors may reuse typed Rule Consequences without turning Action Detail into
choice state. Existing Action workflows retain their distinct command shapes
and collections while sharing the same detail contract.

## User Stories

1. As a Player, I want every offered Formation to explain all consequences of committing it, so that I can make an informed decision.
2. As a Player, I want Action Detail to reflect the current Game State, so that displayed costs and exceptions match the Action I can commit now.
3. As a Player, I want Star Element Substitution shown for the exact Card it affects, so that I understand why the Formation is legal.
4. As a Player, I want optional costs shown before committing, so that I know which later benefits require another decision.
5. As a Player, I want automatic costs and schedules distinguished from optional ones, so that I know what I am committing to.
6. As a Player, I want follow-up choices identified before committing, so that the Action does not surprise me with an unexplained decision.
7. As a Player, I want trusted randomness identified before committing, so that I can distinguish a guaranteed result from a random one.
8. As a Player, I want delayed effects and their timing shown, so that I understand consequences beyond the current resolution.
9. As a Player, I want Rule Module exceptions shown with the main effect, so that enabled modules do not silently alter an Action.
10. As a Player, I want guaranteed, conditional, random, follow-up, and scheduled consequences distinguished, so that wording does not overpromise a final outcome.
11. As a Player, I want known commitments described without hidden opponent information, so that Action Detail cannot leak private state.
12. As a Player, I want undecided random outcomes to remain hidden, so that explanation does not become a prediction channel.
13. As a Player, I want Profession Change details to follow the same completeness rules as Formation details, so that decision aids are consistent.
14. As a Player, I want Activated Profession Ability details to include their current cost, target, declarations, and limits, so that I understand the exact offer.
15. As a Player, I want Spirit Skill details to include their current Spirit Power cost and declared inputs, so that I understand the exact offer.
16. As a Player, I want direct Pouch Secret Strategy options to explain their effects and required input, so that I can choose a strategy knowingly.
17. As a Player, I want Discard Retrieval to explain its current HP cost and resulting Card movement, so that it does not require a separate mental model.
18. As a Player, I want Pass to remain concise, so that a trivial control is not burdened by unnecessary detail machinery.
19. As a Player answering Chain, I want the selected Secret Strategy explained consistently with direct Pouch triggering, so that the same rule has one meaning.
20. As a Player answering a Pending Choice, I want its canonical options and Choice ID owned by the choice lifecycle, so that explanatory content cannot be submitted as an answer.
21. As an observer, I want Action Detail to obey the existing viewer policy, so that observing never reveals private Cards or action constraints.
22. As a reconnecting Player, I want each detail to come from the same snapshot as its Action offer, so that it cannot race a separate detail request.
23. As a rules maintainer, I want rule consequences declared through closed typed variants, so that arbitrary prose cannot hide missing semantics.
24. As a rules maintainer, I want consequence descriptors near the behavior they describe, so that changing a rule makes its Player-facing impact visible locally.
25. As a rules maintainer, I want common consequences reusable across Formations, Abilities, Skills, Secret Strategies, and choices, so that shared mechanics do not become identity-based copies.
26. As a rules maintainer, I want Action Detail assembly centralized, so that ordering, visibility, and completeness invariants have one owner.
27. As a Rule Module maintainer, I want adding a new consequence variant to require an explicit Web renderer, so that an incomplete UI fails during development.
28. As a Web maintainer, I want localized wording and layout owned by the Web, so that presentation concerns do not enter canonical state or replay.
29. As a Web maintainer, I want to render domain facts without Formation-ID or Secret-Strategy-ID prose maps, so that the browser does not duplicate Rules Engine behavior.
30. As a Web maintainer, I want Action Detail embedded in existing Action workflows, so that I do not need another endpoint, cache, or loading state.
31. As a command maintainer, I want submitted commands to omit Action Detail, so that server-side validation remains authoritative over client snapshots.
32. As a catalog maintainer, I want Formation Catalog prose to remain concise and independent, so that a catalog entry is not forced to describe every current state.
33. As an event-feed maintainer, I want event summaries to remain separate, so that this refactor does not conflate pre-commit explanation with historical narration.
34. As a contract maintainer, I want exact camelCase serialization protected by tests, so that typed Rust fields remain valid TypeScript input.
35. As a test author, I want rule facts and localized presentation tested at their own seams, so that failures identify the responsible module.
36. As a future maintainer, I want no silent `rule_text` or `summary` fallback, so that incomplete new Actions cannot appear finished.
37. As a future maintainer, I want the unreleased shallow contract removed atomically, so that the new modules do not carry duplicate compatibility paths.

## Implementation Decisions

- Introduce a closed tagged `RuleConsequence` model owned by the Rules Engine. It must represent costs, immediate effects, follow-up choices, trusted randomness, delayed effects, rule exceptions, substitutions, and the certainty or timing needed to describe them accurately.
- Treat Rule Consequences as Player-visible semantic facts. They are not localized prose, canonical Game Events, resolver instructions, or simulations of final Game State.
- Introduce a required `PlayerFacingActionDetail` containing an ordered collection of Rule Consequences for one offered Action snapshot.
- Generate Action Detail for the querying Player and use only public information plus that Player's own visible information. Do not expose hidden opponent state or undecided random results.
- Attach detail directly to the existing offer DTOs. Do not add a detail lookup endpoint and do not require a single union for all Action workflows.
- Keep the detail out of submitted commands. Existing action identity and required command parameters remain the validation input.
- Have each rule behavior provide reusable consequence descriptors next to its execution semantics. Do not centralize rule knowledge in a Formation-ID or Secret-Strategy-ID presentation table.
- Have one Rules Engine Player-Facing Action Detail module assemble rule descriptors with current state, establish clause order, apply viewer-safe context, and enforce completeness.
- Have one Web Action Detail Presentation module exhaustively render typed consequences into localized wording and layout.
- Cover Formation Use, Profession Change, Activated Profession Ability, Spirit Skill, direct Pouch Secret Strategy offers, and Discard Retrieval. Pass may keep a fixed concise label without rich detail.
- Model `SecretStrategyOption` as a source Card, eligible Secret Strategy, and immediately required input candidates. It may serve direct Pouch triggering or a Chain Pending Choice and is not universally a standalone Action.
- Keep Pending Choice canonical identity, options, answer validation, and continuation outside Action Detail. Pending Choice descriptors may reuse Rule Consequences solely for explanation.
- Describe commitments rather than predict complete resolution. Distinguish guaranteed, conditional, random, follow-up, and scheduled consequences when the distinction affects the Player's decision.
- Permit Formation Catalog information only as supplementary reference. Catalog prose does not satisfy Action Detail completeness and is never a fallback for a missing typed consequence.
- Treat an offered Action without complete detail as a development invariant violation. Adding a new rule or consequence variant must require corresponding Rules Engine facts and exhaustive Web rendering.
- After issue #70 is complete, replace the shallow Web seam atomically: remove playable-action `summary`, Formation presentation `policy`, Formation-ID detail maps, and Secret Strategy prose maps in favor of required typed detail.
- Keep Formation Catalog summaries and public event summaries unchanged because they serve different domain purposes.
- Do not add dual-read, dual-write, compatibility, or migration paths for the unreleased Action Detail contract.

## Testing Decisions

- Test external behavior through the highest relevant module interface rather than asserting private helper calls or source-code structure.
- Add table-driven Rules Engine tests for the Rule Consequences produced by every Action family and representative Rule Module exceptions.
- Add completeness tests proving that every generated pre-commit Action offer contains a complete Player-Facing Action Detail and that no catalog-text fallback is used.
- Add player-scoping tests proving that details include permitted own information while excluding hidden opponent information and undecided random outcomes.
- Add certainty tests proving that conditional, random, follow-up, and scheduled effects are not serialized as guaranteed final outcomes.
- Add Pending Choice integration tests proving that choice descriptors may reuse Rule Consequences without exposing canonical continuations or accepting detail as command input.
- Add serialization contract tests for every Rule Consequence variant and exact camelCase names for multiword Web fields.
- Add TypeScript tests that exercise the Action Detail Presentation module with fixtures for every consequence variant and fail exhaustiveness when a new variant is introduced.
- Add representative composition tests for Echo Cost and scheduling, Plant Earth follow-up behavior, Chain Secret Strategy options, Star Element Substitution, limited uses, and Discard Retrieval.
- Retain focused end-to-end coverage for materially distinct interactions such as Echo and Chain. Do not enumerate every Formation or rule effect through E2E when the same behavior is covered at lower seams.
- Remove tests whose only purpose is preserving deleted `summary`, Formation presentation `policy`, or Web identity-to-prose maps.
- Validate with focused Rust tests, Web unit tests, TypeScript checking, relevant representative E2E flows, full `cargo test`, formatting checks, and diff checks.

## Out of Scope

- Changing Formation, Profession, Spirit Skill, Pouch, Secret Strategy, Discard Retrieval, Echo, or Chain game effects and timing.
- Redesigning the canonical Pending Choice lifecycle, Choice ID, Choice Continuation, or answer command established by issue #70.
- Materializing Pending Choice options or canonical choice payloads before an Action is committed.
- Combining playable Actions, Secret Strategy options, and Discard Retrieval into one universal transport union or command catalog.
- Moving localization, wording, or visual layout into the Rules Engine.
- Replacing canonical Game Events, event-feed summaries, or replay data with Rule Consequences.
- Rewriting Formation Catalog content or requiring catalog descriptions to be state-specific.
- Building a generic runtime rule-description language, plugin schema, or arbitrary prose extension point.
- Persisting Action Detail or accepting it back from the browser as authoritative state.
- Adding backward compatibility for the unreleased `summary` and presentation-policy contract.

## Further Notes

- Governing decision: `docs/adr/0023-separate-action-detail-from-catalog-rule-text.md`.
- The glossary definitions for Rule Consequence, Player-Facing Action Detail, Secret Strategy Option, Formation Catalog, Pending Choice, and Game Event are authoritative.
- Issue #67 fixed Echo's immediate player-facing wording but intentionally remains prior art rather than the target architecture; its Web rule-prose mapping should be removed by this issue.
- Existing action-detail unit tests and Echo and Pouch end-to-end tests provide prior art for presentation and representative interaction coverage.
- Existing dirty worktree changes, especially the in-progress issue #70 implementation, belong to the user or another agent and must be preserved.

## Blocked by

- #70 Deepen the Pending Choice lifecycle
