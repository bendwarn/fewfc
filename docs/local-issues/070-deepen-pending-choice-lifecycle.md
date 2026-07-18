# 70 Deepen the Pending Choice lifecycle

## Triage

ready-for-agent

## Problem Statement

A Player decision currently crosses the Rules Engine, Public View, Web contract,
and browser through several overlapping representations. Canonical Pending
Choice variants mix legacy card-only forms with a generic multi-axis option bag;
their continuations are identified by string pairs; the Web contract flattens
mostly mutually exclusive options into one broad shape; and the browser rebuilds
the legal answer by inspecting `kind` strings and non-empty arrays.

This shallow interface spreads one choice lifecycle across rule dispatch,
projection, serialization, draft state, and presentation. It permits impossible
states, makes stale answers hard to distinguish from current answers, requires
coordinated edits in many callers, and has already allowed a special visual
module to emit `select` while its caller listened for `choose`.

## Solution

Build two deep modules joined by one closed typed Public Pending Choice contract.
The Rules Engine Pending Choice module owns canonical creation, identity,
continuation, validation, lifecycle events, and answer dispatch. The Web Pending
Choice Interaction module owns client-local draft transitions, player-facing
presentation, and construction of the one typed answer command.

Public View filters hidden information before the contract crosses the Web seam.
The answering Player receives one answer-shaped visible variant. Other viewers
receive a hidden variant containing only the answering Player and an
already-public reason for waiting. Initial Pouch Selection remains a distinct
Game Preparation workflow while reusing the Web module's internal Pouch Card
selection implementation with Chain.

## User Stories

1. As the answering Player, I want each Pending Choice to expose only its legal answer shape, so that I cannot submit an impossible combination of fields.
2. As the answering Player, I want Card choices to state their exact minimum and maximum, so that I know when the answer is complete.
3. As the answering Player, I want Player, Formation, Environment, and Decline choices to be explicit variants, so that the controls match the decision being made.
4. As the answering Player, I want Chain and Sheep Stealing to retain named composite decisions, so that their multi-part answers remain coherent.
5. As the answering Player, I want a stale browser answer rejected, so that it cannot answer a later Pending Choice with the same visible shape.
6. As the answering Player, I want reconnect to show the current canonical Pending Choice with a fresh local draft, so that abandoned clicks are not mistaken for submitted decisions.
7. As the answering Player, I want changing to a new Choice ID to clear the old selection draft, so that Cards from the previous decision cannot leak into the next one.
8. As the answering Player, I want one answer command for every canonical Pending Choice, so that Turn Draw and rule-generated choices behave consistently.
9. As a Player selecting an Initial Pouch, I want the familiar Pouch Card interaction reused, so that Initial Pouch Selection and Chain do not drift visually.
10. As a Player selecting an Initial Pouch, I want Game Preparation semantics preserved, so that my selection is not misrepresented as a Formation continuation.
11. As another Player, I want hidden Pending Choice options redacted, so that private Cards and answer constraints are not leaked.
12. As another Player, I want to know which Player is deciding and the already-public reason for waiting, so that the paused game remains understandable.
13. As an observer, I want the same hidden-choice policy as a non-answering Player, so that observation never reveals private options.
14. As a replay viewer, I want canonical Choice Requested and Choice Made events replayed deterministically, so that every decision point has a stable identity.
15. As a rules maintainer, I want each Pending Choice to have one record-local monotonic Choice ID, so that identity does not depend on payload hashes or semantic strings.
16. As a rules maintainer, I want every Pending Choice to carry one typed Choice Continuation, so that invalid effect-ID and continuation-string pairs cannot exist.
17. As a Rule Module maintainer, I want to contribute a typed choice request without allocating identity or constructing final Pending Choice state, so that lifecycle invariants remain local.
18. As a Rule Module maintainer, I want to resume only Choice Continuations owned by that Rule Module, so that continuation dispatch is exhaustive and explicit.
19. As a rules maintainer, I want legacy card-only variants removed, so that one answer-shaped model is authoritative before deployment.
20. As a rules maintainer, I want Choice Requested and Choice Made to own the waiting-state lifecycle, so that domain consequence events do not also have to clear Pending Choice state.
21. As an event-feed maintainer, I want domain-specific consequence events to remain distinct, so that Turn Discard, Echo Cost, and other outcomes retain their game meaning.
22. As a Web maintainer, I want player-facing wording and visual policy outside canonical rules, so that UI concerns cannot enter replay or Game State.
23. As a Web maintainer, I want special visual modules to be internal implementation details of one interaction module, so that their event naming cannot leak across callers.
24. As a TypeScript caller, I want an exhaustive closed union rather than `kind: string`, so that adding a choice variant produces compile-time work at every required presentation branch.
25. As a contract maintainer, I want every multiword action field protected by exact camelCase serialization tests, so that valid browser answers cannot become server errors.
26. As an online-room maintainer, I want retry idempotency keyed by the online command ID, so that response loss does not duplicate a canonical answer.
27. As an online-room maintainer, I want a new command referencing a completed Choice ID rejected as stale, so that command retry and choice identity remain separate concerns.
28. As a test author, I want to test complete choice transitions through module interfaces, so that bugs are found without relying on broad E2E flows.
29. As a test author, I want representative E2E coverage for each special interaction form, so that rendering and submission remain connected without duplicating every rules case.
30. As a future maintainer, I want no compatibility path for the unreleased flattened contract, so that the new deep modules do not carry the old shallow interface indefinitely.

## Implementation Decisions

- Use two deep modules: a Rules Engine Pending Choice module and a Web Pending Choice Interaction module.
- Join the modules with one closed typed Public Pending Choice contract after Public View redaction.
- Give every canonical Pending Choice one record-local monotonic Choice ID that is recorded by Choice Requested and supplied by the answer command.
- Replace effect-ID and continuation-string pairs with typed Choice Continuations. Each Rule Module owns only its continuations.
- Rule Modules submit typed choice requests; the Pending Choice module allocates the Choice ID, validates option invariants, and creates the canonical waiting state.
- Replace legacy effect-generated and card-set variants with answer-shaped choices.
- Do not use a generic option bag containing several independent choice dimensions. Use explicit Card, Player, Formation, and Environment variants, plus named composite variants where one decision genuinely spans several dimensions.
- Treat Decline as an explicit capability of the variants that allow it.
- Use one canonical answer command for Turn Draw Discard and every rule-generated Pending Choice.
- Require the answer command to carry the answering Player, Choice ID, and closed typed answer.
- A mismatched Player, mismatched Choice ID, invalid answer variant, invalid selection count, or disallowed option is a Validation Failure that emits no Game Events and leaves the Pending Choice unchanged.
- Keep online command-ID idempotency in the online command transaction module. A different command ID referencing a completed Choice ID is stale and must fail validation.
- Use Choice Requested to create Pending Choice state and Choice Made to record the typed answer and clear that state.
- Keep domain-specific consequence events separate from the generic lifecycle events.
- Allow one answer command to resolve deterministic consequences until the next interactive Pending Choice, Pending Randomness, or stable Player-ready state. Every subsequent Pending Choice receives a new Choice ID.
- Public View emits a typed visible variant only to the answering Player. Other viewers receive a hidden variant with the Player and only an already-public waiting reason.
- Do not expose the canonical Choice Continuation through Public View.
- The Rules Engine and Public View expose domain facts and typed public context; the Web module owns Chinese wording, control choice, focus behavior, and draft transitions.
- Keep unsubmitted selection drafts client-local. Reset them when the Choice ID or viewer changes and reconstruct an empty draft after reconnect.
- Keep Initial Pouch Selection as Game Preparation, not a Pending Choice. Reuse internal Pouch Card selection implementation between Initial Pouch Selection and Chain without sharing their canonical lifecycle.
- Keep substantial special visual modules internal to the Web interaction implementation. They emit local selection intent; they do not own answer legality or canonical answer construction.
- Replace the old canonical variants and flattened Web contract atomically before deployment. Do not add dual-read, dual-write, migration, or compatibility shims for unreleased behavior.
- Preserve the existing Ruleset, Game Record, Public View, hidden-information, trusted-randomness, Formation, Echo, Pouch, and Turn semantics except where required to express the unified Pending Choice lifecycle.

## Testing Decisions

- Test external behavior through the highest relevant interface. Avoid assertions about private helper calls, source text, or internal module arrangement.
- Add table-driven Rules Engine Pending Choice module tests covering every answer-shaped variant, selection bounds, optional Decline, composite answers, wrong Player, stale Choice ID, wrong answer shape, unavailable options, and zero-event validation failure.
- Add lifecycle tests proving that Choice Requested allocates sequential record-local Choice IDs, Choice Made clears exactly the matching Pending Choice, and replay reconstructs the same IDs and state.
- Add Rule Module continuation tests proving exhaustive typed dispatch and sequential Pending Choices with distinct IDs.
- Retain existing rule integration tests for the domain consequences of Turn Draw, Echo, Tribulation, Pouch, Profession, and other choices.
- Add Public View tests for the answering Player's visible typed variant and every other viewer's hidden variant. Assert that private options, counts, answer shape, and canonical continuation are absent when hidden.
- Add Rust serialization contract tests for every public choice and answer variant, including exact camelCase names for multiword fields.
- Add Web interaction tests that start from each public variant, exercise draft transitions, derive valid and invalid submission states, build the correct typed answer, and reset on Choice ID or viewer change.
- Test Initial Pouch Selection and Chain through the same internal Pouch Card interaction behavior while asserting their distinct canonical commands and outcomes.
- Retain focused tests for special visual behavior such as Card matrices and Formation grouping, but test them through the parent interaction interface when the behavior crosses module seams.
- Keep one representative E2E flow for each materially distinct visual interaction. Do not use E2E to enumerate every rules variant already covered below the Web seam.
- Remove tests whose only purpose is reading the large application source or proving details of deleted shallow helpers.
- Validate with focused Rust tests, related Web tests, TypeScript type checking, the relevant representative E2E flows, full `cargo test`, formatting checks, and diff checks.

## Out of Scope

- Moving Initial Pouch Selection out of Game Preparation or representing it as canonical Pending Choice state.
- Persisting or synchronizing unsubmitted browser selection drafts.
- Exposing canonical Choice Continuations, hidden Cards, or private option constraints to non-answering viewers.
- Building a runtime Rule Module plugin system, public continuation trait, or adapter framework for the closed official catalog.
- Redesigning Pending Randomness, trusted shuffle ownership, or online command transactions beyond the Choice ID interaction described here.
- Changing the game effects or timing of Turn Draw, Echo, Tribulation, Pouch, Profession Abilities, Spirit Skills, or Formations.
- Introducing backward-compatibility support for unreleased canonical events or Web DTOs.
- Refactoring unrelated route modules, Game Room session behavior, Player-Facing Action Detail, Tuning Card Obligation, or Official Rule Module composition.

## Further Notes

- Governing decision: `docs/adr/0026-deepen-the-pending-choice-lifecycle.md`.
- Related decisions: ADR-0015 for typed Pending Choice answers and ADR-0020 for Initial Pouch Selection ownership.
- The glossary definitions for Pending Choice, Choice ID, Choice Continuation, Choice Requested, and Choice Made are authoritative.
- Existing dirty worktree changes outside this issue belong to the user and must be preserved.

## Blocked by

- none
