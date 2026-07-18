---
status: accepted
---

# Deepen the Pending Choice lifecycle across the Web seam

The Rules Engine and Web application use two deep modules joined by one closed
typed Public Pending Choice contract. The Rules Engine Pending Choice module
owns canonical choice creation, a record-local monotonic Choice ID, typed Choice
Continuations, answer-shaped variants, answer validation, one answer command,
and the Choice Requested and Choice Made lifecycle events. Each Rule Module
contributes typed choice requests and resumes only the Choice Continuations it
owns; it does not construct final Pending Choice state or allocate Choice IDs.

Public View filters hidden information before the contract crosses the Web seam.
The answering Player receives one typed visible variant, while other viewers
receive a hidden variant containing only the Player and an already-public reason
for waiting. The Web Pending Choice Interaction module owns client-local draft
transitions, player-facing presentation, and typed answer construction. Drafts
are reset by Choice ID and are not persisted or synchronized through the server.

Initial Pouch Selection remains the distinct Game Preparation workflow chosen in
ADR-0020, even though it and Chain reuse the Web module's internal Pouch Card
selection implementation. The old EffectGenerated and CardSetChoice variants,
the flattened Web DTO option bag, the separate Turn Draw and effect-choice answer
commands, and their compatibility paths are removed before deployment rather
than supported alongside the new contract. Domain-specific consequence events
such as Turn Discard Chosen and Echo Cost Paid remain separate from the generic
Pending Choice lifecycle events.

## Considered options

- One tier-spanning module was rejected because player-facing presentation must
  not enter canonical rules, replay, or Public View redaction.
- A generic multi-axis option bag was rejected because callers had to infer the
  legal answer from whichever Cards, Players, Formations, or Environments happened
  to be present. Composite decisions such as Chain retain named answer-shaped
  variants; sequential decisions create sequential Pending Choices.
- Server-persisted selection drafts were rejected because unsubmitted UI state is
  neither canonical game state nor required for deterministic reconnect.

## Consequences

The Pending Choice module interface is the Rust lifecycle test surface, Public
View tests visible and hidden variants, the Web interaction interface tests
draft-to-answer behavior, and serialization contract tests cover every Rust to
TypeScript variant. Online command-id idempotency remains owned by the command
transaction module; a different command that references a completed Choice ID
is a Validation Failure and emits no Game Events.
