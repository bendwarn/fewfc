# 69 Separate randomness continuations from player responses

## Triage

ready-for-agent

## Problem Statement

A Player can select a legal Rusted Iron Withered Forest Formation and still be
told that the choice is invalid. The Formation is accepted by Formation
matching and correctly creates Pending Randomness, but the Rust Web adapter
then starts building a Player-ready response while the trusted shuffle is still
unresolved. That projection queries Playable Actions, which correctly rejects
gameplay queries during Pending Randomness, and the resulting validation error
is presented to the Player as `選擇無效，請重新選擇。`.

The Player should not observe this trusted application-adapter decision. From
the Player's perspective, one command performs the Formation, the Online Game
Room completes every trusted shuffle internally, and the response arrives at
the next Player decision, such as Turn Draw Discard Choice.

## Solution

Separate trusted-randomness continuation results from Player-ready rules
responses with a closed, tagged result contract. A `needsRandomness` result
contains only the canonical record needed to resume and the trusted randomness
request. A `ready` result contains the Public Game State, public events,
Playable Actions, and interaction affordances required by a Player-facing
response.

The Online Game Room consumes and persists every `needsRandomness` result,
supplies a trusted permutation, and repeats until it receives `ready`. Only a
`ready` result may be projected into a Game Room response or WebSocket room
state. Pending Randomness remains an internal seam between the deterministic
Rules Engine adapter and the trusted Worker.

## User Stories

1. As the performing Player, I want a legal Rusted Iron Withered Forest to be accepted, so that an internal Deck shuffle is not reported as an invalid choice.
2. As the performing Player, I want one command submission to advance through trusted shuffles to the next Player decision, so that implementation continuations are invisible during play.
3. As the performing Player, I want the final response to show Turn Draw Discard Choice when the Formation completes, so that I can continue the Turn normally.
4. As another Player, I want private Deck order and trusted shuffle requests to remain hidden, so that online play does not leak canonical hidden information.
5. As a reconnecting Player, I want an interrupted trusted shuffle to resume safely, so that a Worker restart does not lose or duplicate the Formation.
6. As a room participant, I want each Formation command to be committed exactly once, so that retries do not duplicate canonical events or effects.
7. As a rules maintainer, I want trusted continuation results and Player-ready results to be distinct variants, so that impossible response states are rejected by exhaustive matching.
8. As a rules maintainer, I want Playable Actions to be computed only at a Player decision, so that pending internal work cannot trigger unrelated gameplay validation.
9. As a Worker maintainer, I want each Pending Randomness record persisted before resolving it, so that every continuation remains retryable after failure.
10. As a Worker maintainer, I want sequential shuffle requests drained through one loop, so that Formations with multiple Deck operations cannot expose an intermediate result.
11. As a replay auditor, I want accepted shuffle permutations to remain canonical recorded decisions, so that replay never reruns a PRNG.
12. As a TypeScript caller, I want the Rust and Web tagged contracts to use exact camelCase serialization, so that every result variant is decoded consistently.
13. As a test author, I want to assert the Game Room command response rather than internal UI rendering details, so that the regression is locked at the highest Player-observable seam.
14. As a local developer, I want the captured room record to remain a read-only reproduction artifact, so that diagnosis never mutates or publishes hidden room data.

## Implementation Decisions

- Replace the optional Pending Randomness field on the otherwise Player-ready response with a closed tagged result contract.
- Use the camelCase discriminants `needsRandomness` and `ready` in both Rust serialization and TypeScript types.
- `needsRandomness` carries the updated canonical record and one trusted randomness request. It does not carry Public Game State, public events, Playable Actions, or interaction affordances.
- `ready` carries the existing completed rules response used for Public Game State, public events, Playable Actions, interaction affordances, and trusted hand-candidate queries.
- The Rust Web adapter returns `needsRandomness` immediately when command resolution reaches Pending Randomness. It must not call Player-response projection or query Playable Actions first.
- The trusted Worker exhaustively matches the result variants. It persists the updated record for every `needsRandomness`, submits an exact trusted permutation, and continues until the result is `ready`.
- The shared sequential-randomness helper accepts the tagged result contract and returns only the ready variant after draining zero or more requests.
- Callers that cannot legitimately create Pending Randomness must explicitly require `ready`; callers that execute gameplay commands capable of randomness must use the draining path.
- Game Room HTTP responses and WebSocket room-state messages are ready-only contracts. They never serialize a trusted randomness request, Deck order, or continuation discriminator to a Player.
- Existing Pending Command Draft retry behavior remains intact. A failure after persistence leaves the same randomness request retryable without committing the Player command twice.
- Canonical command, event, Pending Randomness, and recorded-decision formats do not change. No room-record migration is required.
- Rusted Iron Withered Forest rules, Divine Calculation protection, Tailwind recovery, trusted permutation validation, and replay behavior remain unchanged.
- Do not solve the defect with only a `pending_randomness.is_none()` projection guard. Such a guard may be defensive, but the tagged contract is the required primary design.

## Testing Decisions

- Prefer observable result contracts over implementation-call assertions. The highest Player-facing seam is one Online Game Room command submission returning the next Player decision.
- Add a Rust Web adapter regression in which a Player-view legal Rusted Iron Withered Forest returns `needsRandomness`. Assert its exact camelCase discriminator and request fields, and assert that Player-ready fields are absent.
- Add a Rust serialization contract test for both tagged variants. This protects the multiword trusted-randomness fields and exact camelCase contract consumed by TypeScript.
- Update the existing shared Worker randomness-sequence test to cover one and multiple consecutive `needsRandomness` variants, persistence before each resolution, exact resolve actions, and a ready-only result.
- Add a Game Room regression that submits a legal Rusted Iron Withered Forest once, allows the Worker to complete trusted randomness internally, and receives Turn Draw Discard Choice without exposing a randomness continuation.
- The Game Room regression must assert that the Player command is committed once and that the canonical events include the trusted shuffle and completed Formation sequence.
- Use a deterministic Rules Engine-owned fixture for committed tests. Do not depend on the mutable Wrangler room database or copy unrelated hidden Cards into a fixture.
- Retain the existing focused Tribulation test for revealing, discarding, waiting for trusted shuffle, and completing the Formation after a valid permutation.
- Retain Pending Randomness permutation, stale-request, retry, replay, and camelCase tests as prior art from the completed midgame-randomness work.
- Validate with focused Rust Web adapter and Tribulation tests, related Bun shared/Worker tests, full `cargo test`, and TypeScript type checking. Run broader Web build or Brave coverage if the changed shared response types affect those paths.

## Out of Scope

- Changing the source or algorithm used by the trusted Worker to shuffle Cards.
- Moving entropy generation into the deterministic Rules Engine.
- Changing Rusted Iron Withered Forest, Divine Calculation, Tailwind, Turn Draw, or Formation matching rules.
- Changing canonical events, recorded decisions, replay semantics, or persisted room schemas.
- Exposing trusted randomness requests or Deck order to browser clients.
- Repairing or advancing the captured local room record.
- Broadly redesigning Card-selection Formation queries beyond separating internal continuation results from Player-ready responses.

## Further Notes

The read-only reproduction is room
`5b9a7371-83e4-4f83-9abf-8e32230944fc` after canonical command 79. Glimmer
interprets Card Instance 14 as effective level three. Card Instances
`[4, 22, 12, 14]` are then the unique legal Rusted Iron Withered Forest:
Metal levels total seven and Wood effective levels total seven.

Replaying that exact command as the Player deterministically returns
`Validation::PendingRandomnessInProgress`. Replaying the same state through a
non-Player projection exposes the intended trusted request; resolving its exact
permutation completes Rusted Iron Withered Forest, consumes Divine Calculation,
changes the defending Team HP from 133 to 93, and advances to Turn Draw Discard
Choice. This proves that Formation matching, the shuffle request, the
continuation, and replay are correct; only the mixed continuation/Player
response interface is defective.

The governing decision is ADR 0014, which requires a trusted application
adapter to supply midgame shuffle outcomes while replay consumes the recorded
order. Completed local issues 62 and 66 are prior art for Pending Randomness and
Tribulation respectively.
