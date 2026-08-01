# 73 Deepen Online Command Transactions

## Triage

ready-for-agent

## Problem Statement

The Online Game Room currently spreads one Player command across the HTTP
route, browser composable, `GameRoom` Durable Object, Rules Engine adapter,
Pending Command Draft storage, canonical room events, snapshots, and
notifications. The browser does not provide a stable Command ID, intermediate
choice resolution is held in a second snapshot inside Pending Command Draft,
and the final event, sequence, snapshot, metadata, and draft cleanup are
written as separate awaited operations.

This makes response-loss retries unsafe, permits partial canonical commits,
and treats a multi-step resolution as though it belongs only to the Player who
submitted the root Action. That ownership is incompatible with canonical
Pending Choices that legitimately continue through another Player. The
Durable Object's in-memory request queue reduces concurrency but is not a
transaction boundary and must not be the only protection against stale writes.

## Solution

Introduce one deep Online Command Transaction module with a single primary
`executePlayerCommand` entry point. The module owns authenticated Player
binding, Game Instance and command identity, idempotency receipts, resolution
transaction correlation, Rules Engine execution, canonical waiting
checkpoints, trusted-randomness continuation, optimistic sequence validation,
and atomic canonical commits.

The canonical Game Record remains the only authoritative game state. Remove
Pending Command Draft and never place another Game Record or snapshot inside
transaction metadata. Every accepted Player Command and every externally
waiting Pending Choice or Pending Randomness becomes a durable canonical
checkpoint immediately. A resolution Transaction correlates those checkpoints;
it is not a long-lived database transaction.

## User Stories

1. As a Player, I want retrying after a lost response to apply my command exactly once, so that reconnect and network failure cannot duplicate game effects.
2. As a Player, I want a retry to return the latest authoritative room state, so that I do not receive a stale UI snapshot captured by the original response.
3. As a Player, I want a reused Command ID with different input rejected, so that an idempotency collision cannot perform the wrong Action.
4. As a Player answering a Pending Choice, I want authorization based on that choice's canonical Player, so that a resolution can legitimately continue across Players.
5. As a reconnecting Player, I want an accepted Action waiting on a mandatory choice to remain resumable, so that disconnect does not cancel canonical game progress.
6. As a room participant, I want a failed canonical write to commit nothing, so that event history, sequence, snapshot, and transaction state cannot disagree.
7. As a room participant, I want notification failure to leave an already committed command successful, so that delivery concerns cannot roll back game history.
8. As a replay auditor, I want every accepted command and trusted randomness answer correlated with stable identities and committed sequence numbers, so that resolution history can be explained without rerunning entropy.
9. As a rules maintainer, I want the Game Record to remain the sole authority for Pending Choice and Pending Randomness, so that transaction recovery cannot select between competing snapshots.
10. As an online-room maintainer, I want deterministic Validation Failures distinguished from retryable infrastructure and implementation failures, so that Command IDs have predictable retry behavior.
11. As a test author, I want to exercise complete transactions through one module interface, so that atomicity and recovery can be verified without exposing orchestration helpers.
12. As a future maintainer, I want a clean replacement of the unreleased transaction schema, so that the module does not carry legacy Pending Command Draft migration paths.

## Implementation Decisions

- Give every new game one immutable `gameInstanceId`. A reset starts a new Game Instance and invalidates delayed commands from the previous game.
- Have the browser generate one stable Command ID before each Player submission and reuse that ID only when retrying the identical submission. Do not generate the normal browser Command ID inside the server route.
- Give the accepted root Action one Transaction ID. Keep that Transaction ID across all Pending Choice and Pending Randomness continuations until the canonical Game Record reaches a stable Player-ready state.
- Bind a Command ID to its Game Instance, Transaction, authenticated actor, and exact validated Action payload. The payload identity includes the Choice ID, answer, Cards, targets, and every other submitted parameter.
- Treat the same Command ID with identical identity as a duplicate. Treat any actor, Transaction, Game Instance, or payload mismatch as an idempotency conflict.
- Persist an immutable receipt for every accepted Player Command. It records at least the Command ID, Transaction ID, actual authenticated actor, committed sequence, payload identity, and transaction status at that checkpoint.
- Return the latest authoritative room state plus the immutable receipt for a duplicate retry. Idempotency guarantees the same effect, not a byte-identical stale response.
- Persist deterministic Validation Failures as noncanonical rejection receipts bound to the Command identity, observed sequence, and stable error code. They emit no Game Events, do not mutate Game State, and do not advance the canonical sequence. A retry with the same identity returns the same rejection even if room state has since changed.
- Do not create terminal receipts for Rule Implementation Error, Engine Invariant Error, storage failure, network failure, or other 5xx outcomes. The same Command ID may retry after the underlying fault is corrected.
- Use one open resolution Transaction per room. Record the root actor for correlation, but authorize every continuation from canonical state: Pending Choice uses `PendingChoice.player`, while Pending Randomness is accepted only from the trusted application adapter.
- Make the canonical Game Record the only authority for the transaction's current waiting state. Derive `awaitingChoice`, `awaitingRandomness`, or `complete` from that record rather than maintaining a separately mutable status. Both waiting states being present together is an Engine Invariant Error.
- Commit every accepted Player Command immediately, including one that creates another Pending Choice. Persist Pending Randomness before invoking the trusted adapter, and commit every trusted answer before proceeding to a later randomness request or stable Player-ready state.
- Reuse the Rules Engine's canonical `PendingRandomness.requestId` and typed trusted-randomness answer. The Rules Engine receives and validates the answer; it never calls an entropy source internally. Do not duplicate randomness identity or pending state in transaction metadata.
- Require every canonical commit to carry `expectedSequence`. Atomically verify that sequence and write the canonical events, next sequence, Game Record snapshot/checkpoint, room metadata, transaction correlation, and Command or trusted-decision receipt. A mismatch writes nothing and returns a stale-write conflict.
- Implement the atomic boundary with the SQLite-backed Durable Object storage owned privately by the module. Keep Durable Object serialization as coordination and optimization, but do not rely on the class-level promise queue for correctness.
- Remove `PendingCommandDraft` and its duplicated snapshot. Transaction metadata contains only identities and correlation facts that are not already authoritative in the Game Record.
- Expose one primary `executePlayerCommand` interface. Keep receipt lookup, actor binding, Rules Engine calls, randomness draining, sequence checks, commits, and recovery internal to the module.
- Keep HTTP decoding, authentication transport, read-only room queries, WebSocket broadcast, and notifications outside the transaction module. Broadcast and notifications run only after commit and cannot change the transaction result.
- Do not add generic cancellation or timeout. An unsubmitted browser draft is client-local; an optional canonical choice may decline only through its typed `Decline` answer. Mandatory choices remain open across disconnect. Future timeout, auto-answer, or forfeit behavior must be an explicit rule and canonical Command.
- Retain receipts only for the matching Game Instance's replay lifetime. Commands must carry `gameInstanceId`, so a delayed command from a discarded instance fails with `gameInstanceMismatch` and cannot affect the new game.
- Replace the current persisted room transaction shape atomically. Do not add dual-read, dual-write, migration, or compatibility behavior for old rooms or legacy Pending Command Drafts.
- Do not introduce a public generic storage repository for hypothetical adapters. Use module-private seams only where they enable transaction-level tests and fault injection.

## Testing Decisions

- Test the module primarily through `executePlayerCommand` as a black-box contract. Avoid assertions about private helper calls, source layout, or the internal storage representation.
- Add a response-loss test proving that retrying the same Command ID and payload applies the effect once and returns the latest room state with the original receipt.
- Add identity tests for same-ID actor, Game Instance, Transaction, and payload mismatches, including a new Command ID that references an already completed Choice ID.
- Add transaction tests proving that every accepted command which creates Pending Choice is immediately durable and that a later authorized Player can continue the same Transaction.
- Add trusted-randomness recovery tests covering persistence before entropy, Worker restart, repeated request identity, multiple sequential requests, validated answers, and canonical replay without rerunning entropy.
- Add fault injection at each canonical write boundary and prove that event, sequence, snapshot, metadata, and receipts either all commit or all remain unchanged.
- Add an `expectedSequence` conflict test proving that stale computation writes nothing even when the outer Durable Object request queue is bypassed by the test seam.
- Add rejection tests proving that Validation Failure receipts are stable and noncanonical, while Rule Implementation Error, Engine Invariant Error, and infrastructure failures remain retryable with the same Command ID.
- Add a post-commit delivery test proving that WebSocket or notification failure cannot change a successful transaction response or duplicate canonical effects.
- Protect all new Rust-to-TypeScript multiword fields with exact camelCase serialization contract tests.
- Keep only representative HTTP and Durable Object integration coverage above the module. Do not repeat every transaction branch in broad E2E tests.
- Validate with focused Web tests, related Rust command and replay tests, TypeScript checking, full `cargo test`, formatting checks, and diff checks.

## Out of Scope

- Changing Formation, Pending Choice, Pending Randomness, or other game-rule semantics.
- Redesigning the typed Pending Choice lifecycle or browser interaction module established by issue #70.
- Moving entropy generation into the deterministic Rules Engine or accepting Player-provided shuffle order.
- Persisting unsubmitted browser drafts.
- Adding generic transaction cancellation, disconnection cancellation, timeout, auto-answer, or forfeit rules.
- Moving room creation, joining, access control, read-only queries, broadcasting, or notifications into the transaction module.
- Building a generic transaction framework or storage abstraction for other applications.
- Migrating, preserving, or repairing rooms stored with the legacy transaction schema.

## Further Notes

- ADR-0014 remains authoritative for trusted midgame randomness and deterministic replay.
- ADR-0026 and completed issue #70 remain authoritative for Choice ID, typed Pending Choice answers, canonical choice lifecycle events, and stale-choice validation.
- Completed issue #69 is prior art for the ready-versus-needs-randomness Rules Engine adapter contract.
- Game Record, Pending Choice, Pending Randomness, Validation Failure, Rule Implementation Error, and Engine Invariant Error retain their glossary meanings in `CONTEXT.md`.
- Existing dirty worktree changes outside this issue belong to the user and must be preserved.

## Blocked by

- none
