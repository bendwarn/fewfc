# 76 Deepen Secret Strategy Decisions

## Triage

ready-for-agent

## Goal

Replace Secret Strategy's wide optional-field command and option bags with one
deep, closed Decision module that preserves Pouch and Chain behavior while
making invalid input combinations unrepresentable and stale submissions fail
atomically.

## Problem Statement

Direct Pouch triggering and Chain currently spread Secret Strategy offer
construction, optional input interpretation, validation, effect dispatch, and
continuations across the Pouch Rule Module, Web DTOs, and browser drafts. The
command can carry unrelated optional fields, some strategies silently ignore
irrelevant data, and the Web layer must infer which combinations are meaningful.
This weakens the Rules Engine authority and makes new branches easy to add
without exhaustive validation.

The current direct 瞞天 path also accepts a nonexistent Star target, reveals and
consumes the Pouch, and resolves no effect. The published rule requires an
existing Star, so this is an atomic-validation defect rather than a legal
no-effect case. 走為 has different intentional no-change cases that must remain
legal. 牽羊 is structurally deferred and must retain its existing typed Pending
Choice instead of forcing future Card selections into the initial command.

## Solution

Create an internal Secret Strategy Decision child module inside the Rust Pouch
Rule Module. Give it responsibility for answer-shaped offer construction,
complete Decision validation, immediate strategy effect events, and
strategy-specific continuations. Keep reveal, placement, consumption,
direct-versus-Chain lifecycle, and Chain post-search shuffle in the outer Pouch
module.

Use a closed algebraic Command/Parameter Object with exhaustive typed dispatch.
Direct Pouch triggering carries one complete Decision. Chain uses a closed
`PlaceOnly` or `PlaceAndTrigger` envelope, with the latter nesting the same
Decision. Adapt browser drafts to those DTOs without reimplementing Rules Engine
eligibility. Replace the old contract atomically; add no compatibility path.

## Confirmed Decision Shape

A Secret Strategy Decision retains its source Card and exactly one
answer-shaped selection from these five families:

1. No extra input: 金蟬, 偷梁, 混水, 觀火, 還魂, or 暗渡.
2. Target Player: 離山 with exactly one target Player.
3. Star operation: 瞞天 with `Gain(star)` or `Break(star)`.
4. Environment operation: 走為 with `Clear` or
   `TransferByDiscard(card)`.
5. Begin deferred exchange: 牽羊 with no Deck or Discard Pile selections.

The concrete Rust naming may follow repository conventions, but the type system
must enforce these relationships. Do not retain a common bag containing
`target`, `star`, `breakStar`, `discardCard`, `deckCards`, or `discardCards`.
Replace `star + breakStar` with the Star operation and optional discard Card
with the Environment operation. Use one exhaustive internal dispatch rather
than ten shallow Strategy implementations.

Direct Pouch triggering and Chain share this Decision type but keep distinct
outer command or answer variants and lifecycle handling. The source Card is
part of the Decision in both paths. There is no opaque offer or Option ID.

## Offer And Projection Decisions

- Replace generic Secret Strategy options with the same five answer-shaped
  families.
- A no-input option carries only its Secret Strategy.
- A target-Player option carries only currently legal target candidates.
- A Star option carries only currently legal `Gain` and `Break` operations.
- An Environment option carries only currently legal `Clear` and
  `TransferByDiscard` operations.
- A 牽羊 option states only that the deferred exchange can begin.
- Remove unrelated empty collections and generic `requiredCardCount` fields.
- Keep these options in the private interaction/action projection. Do not put a
  submitted Decision in canonical Game State, Public State, or the Public Event
  Feed.
- Serialize Rust-to-TypeScript tagged variants and multiword fields with exact
  camelCase contracts and add explicit serialization tests.

## Validation And Error Decisions

- Revalidate the full Decision from current canonical state at submission. Do
  not trust the earlier offer snapshot.
- Validate the source Card against the direct current Pouch or Chain trigger
  selection, the strategy against the source Card's printed element or level,
  the actor and target authority, and all operation-specific Card or rule state.
- Complete every immediately decidable validation before emitting any
  canonical event.
- A stale, forged, mismatched, or illegal Decision is a Validation Failure. It
  emits no reveal, consume, movement, cost, or other canonical event.
- Only a self-produced Decision that passed validation but cannot be resolved
  is a Rule Implementation Error.
- Do not add an external plug-in seam or change the public `OfficialRules`
  interface.

## Strategy-Specific Decisions

### Deceive Heaven (`瞞天`)

- `Break(star)` may target any Star that currently exists, including one owned
  by either Team.
- If the selected Star no longer exists at submission, return a Validation
  Failure with zero canonical events. Do not reveal or consume the Pouch.
- `Gain(star)` retains the existing Temporary Star Effect semantics.
- Do not change the later legal no-effect case where a Formation granted by a
  Temporary Star Effect tries to break a same-named Star its Team does not own.

### Retreat (`走為`)

- `Clear` is legal when no Environment exists. Reveal and consume the Pouch,
  but emit no Environment-clearing event.
- `TransferByDiscard(card)` requires a current Card Instance in the triggering
  Player's hand and reads its printed element.
- An Exposed Foreign Card is a legal discard and returns to its Card Origin
  Discard Pile.
- Transfer is legal when no Environment exists and when the target element is
  the same as the current Environment.
- Preserve the current transfer event semantics and validate before reveal.

### Sheep Stealing (`牽羊`)

- The initial Decision begins the deferred flow and contains no Deck or Discard
  Pile Card selections.
- If fewer than two Cards are in the Deck and the Discard Pile is nonempty,
  preserve the preliminary Discard Shuffle and its replayable continuation.
- Preserve the later typed Sheep Stealing Pending Choice with exactly two Deck
  selections followed by exactly two selections from the projected Discard
  Pile.
- Preserve ordered Deck-to-Discard projection before validating the return
  selection, including the ability to return newly discarded Cards.
- Keep the source Card set aside and ineligible until the exchange and Deck
  Shuffle finish.
- Preserve the rule that Sheep Stealing's Deck Shuffle satisfies Chain's
  post-search shuffle requirement.

## Canonical Lifecycle And Ordering

- Direct valid immediate resolution: `PouchRevealed`, strategy effect events,
  then `PouchConsumed`.
- Chain `PlaceAndTrigger`: `ChoiceMade`, `PouchPlaced`, `PouchRevealed`, strategy
  effect events, then `PouchConsumed`, followed by the ordinary post-search
  shuffle when required.
- Chain `PlaceOnly`: preserve the existing placement and post-search lifecycle
  without constructing a Secret Strategy Decision.
- Deferred 牽羊 reveals before its continuation and consumes only after the
  exchange and trusted Deck Shuffle complete.
- Preserve Card Origin behavior for direct Pouches, Chain trigger Cards,
  replaced Pouches, and Exposed Foreign Cards.
- Replay must project the same state and event order as direct execution.

## Web Boundary

- TypeScript owns only a browser-local draft-to-Decision adapter and display of
  answer-shaped options.
- Do not reproduce strategy eligibility, target legality, printed-value checks,
  or stale-state validation in TypeScript.
- Replace the old wide action and Chain-answer payloads with exhaustive tagged
  DTO variants. Handle every variant exhaustively in the Web adapter.
- Preserve current player-facing Pouch and Chain interactions unless the new
  typed shape requires a presentation-only adjustment.

## Hard Cutover

- Remove the old optional-field Secret Strategy command, option, action draft,
  and Chain answer shapes in the same change.
- Add no dual reads, deprecated variants, fallback parsing, saved-draft
  migration, or canonical replay compatibility branch for those noncanonical
  Decisions.
- Preserve existing canonical event shapes unless a proven implementation need
  requires a semantic event change. Do not serialize the Decision into events.
- Do not create a new ADR. Decision 42 in `docs/rules-engine-decisions.md` owns
  this implementation decision.

## Testing Decisions

- Before deleting or consolidating an existing Pouch test, inventory every
  assertion claim and name its replacement evidence as required by ADR-0032.
- Preserve `tests/pouch_strategy_interaction_matrix.rs` and the other Pouch
  interaction matrices. Extend them where the refactor crosses an existing
  interaction boundary.
- Add focused Rust tests for each of the five Decision families, direct and
  Chain envelopes, source/strategy mismatch, irrelevant-input
  unrepresentability, stale targets, atomic failure, canonical event order,
  Card Origin, and direct-versus-replay equivalence.
- Add a regression test proving 瞞天 `Break` of a nonexistent Star is a
  Validation Failure with no events, reveal, or consume.
- Add 走為 coverage for `Clear` without Environment, transfer without
  Environment, same-element transfer, ordinary hand Card, and Exposed Foreign
  Card returning to Card Origin.
- Preserve and extend 牽羊 coverage for preliminary Discard Shuffle, ordered
  projected selection, returning newly discarded Cards, source exclusion,
  Chain shuffle reuse, consumption timing, and replay.
- Add Rust serialization contract tests for every changed tagged DTO and exact
  camelCase multiword field.
- Update Web unit/integration tests for answer-shaped options, exhaustive draft
  conversion, direct submission, and both Chain envelopes.
- Retain representative Pouch Playwright coverage; it must not become the sole
  Rules Engine evidence.
- Run focused Rust and Web tests first, then full `cargo test`, relevant Web
  tests, type checking, formatting, and diff checks.

## Out Of Scope

- Changing any Secret Strategy's published effect beyond the confirmed 瞞天
  validation correction and preservation of 走為 no-change cases.
- Moving reveal, placement, consumption, direct-versus-Chain lifecycle, or
  Chain post-search shuffle ownership out of the Pouch Rule Module.
- Replacing 牽羊's later typed Pending Choice with initial command fields.
- Publishing Decisions in Public State or the Public Event Feed.
- Adding a public extension or plug-in system for Secret Strategies.
- Redesigning generic Pending Choice, trusted randomness, Pouch knowledge, or
  Card Origin.
- Creating a new ADR.

## Governing Context

- `CONTEXT.md` defines Secret Strategy Decision, Secret Strategy Option, Pouch,
  Card Origin, Pending Choice, and related ubiquitous language.
- `docs/rule.md` owns Secret Strategy legality and observable consequences.
- Decision 39 owns the Pouch execution model; decision 42 owns this deep-module
  cutover.
- ADR-0014 owns trusted randomness and deterministic replay.
- ADR-0026 owns typed Pending Choice lifecycle boundaries.
- ADR-0032 requires claim-by-claim interaction-test preservation.
- The repository `AGENTS.md` requires exact camelCase Rust-to-TypeScript
  contracts for new multiword action fields.

## Blocked by

- none
