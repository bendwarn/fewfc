# 62 Generalize choices and midgame randomness

## Triage

ready-for-agent

## What to build

Prepare the Rules Engine, recorded-decision model, Web DTOs, and Online Game
Room adapter for Echo's non-Card choices and trusted midgame Deck shuffles.
Replace the card-only effect-choice answer with a closed typed contract, and
introduce retryable Pending Randomness that records complete shuffle outcomes
without admitting client-controlled Deck order or replay-time RNG.

## Acceptance criteria

- [x] Effect-generated choices accept closed typed answers for Cards, Player, Formation, and Decline; every Pending Choice kind declares and validates its accepted answer variants.
- [x] Existing multi-Card continuations such as Chaos retain their exact behavior after migration from the card-only answer payload.
- [x] Invalid answer variants, counts, identifiers, or Cards are Validation Failures that emit no events and do not mutate Game State.
- [x] Rust and Web DTO fields use the exact camelCase contract, with serialization contract tests for every new multiword action field.
- [x] Pending Randomness is serialized separately from Pending Choice, blocks gameplay commands and automatic advancement, and survives replay, persistence, reconnect, and Worker restart.
- [x] Only a trusted application adapter may answer Pending Randomness; public command APIs never accept a client-provided shuffled order.
- [x] A shuffle answer must be an exact permutation of the requested current Deck remainder; missing, duplicate, foreign, or stale Cards fail without mutation.
- [x] Canonical events record the complete accepted shuffled order, and replay consumes that order without rerunning or depending on a PRNG implementation.
- [x] Adapter failure leaves the request retryable and cannot advance a continuation, Turn Start, or Main Phase prematurely.
- [x] One continuation may safely require sequential trusted shuffle decisions, including empty-Deck Discard recycling followed by a second post-search shuffle, without merging or skipping canonical decisions.
- [x] Existing stored commands and active games remain readable through an explicit serde-compatible migration or backward-compatible payload path.
- [x] Focused Rust and Web tests cover every answer kind, invalid payloads, shuffle validation, replay verification, retry, reconnect, and hidden shuffle data.

## Validation

- `cargo test` and strict Clippy pass, including typed Chaos and
  `tests/choices_randomness.rs`.
- Web unit tests (28), Nuxt type checking, and the production build pass.
- The complete self-contained Brave suite passes: 19 of 19.
- The trusted adapter persists each Pending Randomness request before resolving
  it and its sequence test covers two distinct shuffles in one continuation.
- Public room DTOs expose only request metadata and Card count; the complete
  current and shuffled orders remain internal or canonical.

## Blocked by

None - can start immediately
