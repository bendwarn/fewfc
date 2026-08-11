# 75 Allow Independent Initial Pouch Selection

## Triage

ready-for-agent

## Goal

Let every outstanding Player choose their initial Pouch immediately while
preserving deterministic replay, hidden information, safe Online Game command
commits, and a deliberate hard cutover from legacy games and Replays.

## Problem Statement

Initial Pouch Selection currently stores one expected Player in Game
Preparation, validates choices in Turn Order, emits the next Player in
`InitialPouchChosen`, and exposes only that Player's chooser in the Web UI. The
choices are actually independent: each Player removes one Card from their own
Personal Deck and places it in their own Pouch. Requiring Turn Order therefore
adds waiting without protecting a shared rule invariant.

The Online Game Room already serializes commands and atomically checks the
expected canonical sequence, so near-simultaneous submissions do not require a
new concurrent-write mechanism. Its HTTP and WebSocket delivery can still
apply an older active-game snapshot after a newer one because broadcasts are
post-commit asynchronous and room responses have no active-record sequence.

The canonical preparation shape and event contract are changing. The release
will therefore discard old active Game Records and all completed Replays rather
than add compatibility paths. Waiting rooms must survive the cutover.

## Solution

Make Initial Pouch Selection one shared Game Preparation stage. Derive its
outstanding Players from canonical Pouches, accept one independent command from
each Player in server arrival order, and close the stage through an explicit
completion event only after every Player has chosen. Keep trusted shuffles and
the initial deal ordered by Turn Order.

Expose public completion progress without Card identity, keep the existing
immediate-selection interaction, and version only active-game room snapshots so
the browser can reject stale delivery for the same Game Instance. Add an
idempotent, protected management CLI for the one-time legacy purge. Do not add a
D1 migration or a Durable Object class migration, and do not run the purge as
part of ordinary deploys.

## User Stories

1. As a Player, I want to choose my initial Pouch as soon as the game starts,
   so that I do not wait for another Player's setup decision.
2. As a Player who has chosen, I want to see that I am waiting for the remaining
   Players and retain private knowledge of my selected Card after reconnect.
3. As a room participant, I want to see who has completed or remains without
   learning another Player's Pouch identity.
4. As a Player using another tab, I want a second new submission rejected as
   already chosen without changing the game.
5. As a Player retrying after response loss, I want the same Command ID to
   return its existing receipt without placing another Pouch.
6. As a replay auditor, I want accepted choices recorded in actual server
   arrival order and the same final state reconstructed for every legal order.
7. As a reconnecting Player, I want an older active-game broadcast ignored when
   a newer snapshot for the same Game Instance was already applied.
8. As an operator, I want one explicit, dry-run-first CLI to clear incompatible
   games and Replays without deleting accounts, Deck Lists, or waiting rooms.
9. As an operator, I want a partial cleanup failure to leave the application in
   maintenance so I can safely rerun the same purge epoch.

## Confirmed Rules Semantics

- Apply the policy to every supported capacity: two Players and four Players in
  team mode. Do not introduce a two-Player special case.
- `GamePreparationStage::InitialPouchSelection` no longer stores a Player.
- Outstanding Players are Turn Order minus owners of Pouches already placed
  during Initial Pouch Selection. Do not duplicate this as canonical
  `remainingPlayers` state.
- Any outstanding Player may submit `ChooseInitialPouch`. Validate membership
  in Turn Order, absence of an existing initial Pouch, and ownership of the
  selected Card in the Player's current Personal Deck.
- A successful choice is immutable. A new Command ID after that Player chose
  returns `InitialPouchAlreadyChosen` and emits zero canonical events.
- Emit `PouchPlaced`, then `InitialPouchChosen { player, card }`. Remove
  `next_player` from the canonical event.
- The final outstanding choice also emits `InitialPouchSelectionCompleted`,
  then the first `RandomnessRequested`. Only the completion event advances the
  stage to `PendingDeckShuffle`.
- Continue shuffling remaining Personal Decks and dealing initial hands in Turn
  Order. Generate exactly one first shuffle request after the completion
  barrier.
- The canonical event log keeps server arrival order. Independent choice
  batches must project the same final state in every legal order.
- Initial Pouch Selection remains Game Preparation, not Pending Choice. Each
  non-final choice is an independently completed Online Command Transaction;
  do not hold one open multi-Player transaction. The final choice may enter the
  existing trusted-randomness continuation.
- Do not add timeout, automatic choice, cancellation, or reselection.

## Public View And Web Decisions

- Replace the single-player preparation projection with this exact camelCase
  public contract:

  ```ts
  initialPouchSelection: {
    remainingPlayers: PlayerId[]
  } | null
  ```

- Order `remainingPlayers` by Turn Order. Derive completed Players in the Web
  adapter as Turn Order minus this list; do not transmit both lists.
- Keep `interaction.canChooseInitialPouch` viewer-specific. It is true only
  during Initial Pouch Selection when the viewer is outstanding.
- Show completion and remaining-player progress publicly. Never include chosen
  Card identity in the progress object or another viewer's event projection.
- Preserve Pouch knowledge: the owner can inspect their chosen Card immediately
  and after reconnect; everyone else sees a Card Back until ordinary reveal
  rules make it public.
- Both Players initially see the chooser. A Card click submits immediately with
  no confirmation. The existing `R` random shortcut also submits immediately.
- After a successful choice, close that Player's chooser and show the waiting
  state. Treat `InitialPouchAlreadyChosen` from another tab as reconciliation:
  refresh canonical state and move to waiting without a persistent alarming
  error.
- Add this optional active-game response contract, present whenever the response
  is backed by a current Game Record:

  ```ts
  activeGameVersion?: {
    gameInstanceId: string
    recordSequence: number
  }
  ```

  For the same `gameInstanceId`, ignore HTTP or WebSocket responses whose
  `recordSequence` is lower than the highest already applied. Do not add
  `roomRevision`, and do not use this viewer guard as a canonical concurrency
  boundary.
- Add exact Rust-to-TypeScript camelCase serialization contract tests for every
  new multiword field, especially `initialPouchSelection`, `remainingPlayers`,
  and the public completion event.

## Online Command Decisions

- Keep the Durable Object request queue and `expectedSequence` atomic commit
  boundary. They linearize near-simultaneous commands and prevent lost updates.
- Do not reserve a Turn Order slot, normalize event order after commit, roll
  back a valid earlier choice, or create multiple Pending Choices.
- Preserve normal Command ID behavior: the same ID and identity returns the
  immutable receipt; a different identity under the same ID is an idempotency
  conflict.
- Ensure two nearly simultaneous distinct Players both commit exactly once.
  Whichever commit observes the completed barrier owns creation of the first
  trusted shuffle request.
- Keep post-commit broadcast and notification outside the canonical
  transaction. The active-game response version protects the browser from
  delivery reordering for the same Game Instance.

## Hard Cutover And Management CLI

- Do not add old Game Record reads, event fallbacks, dual-write, repair, or
  replay compatibility. Do not preserve completed legacy Replays.
- Do not add or change D1 migrations. Do not add a Durable Object class
  migration. Keep the existing GameRoom and ReplayArchive classes and
  namespaces so the new version can create new games and Replays normally.
- Add a protected, idempotent GameRoom management operation such as
  `purgeLegacyGameData(epoch)`. It must preserve room ID, name, access, owner,
  members, seats, invitation, capacity, ruleset, and enabled Rule Modules.
- Preserve an already Waiting room's readiness and Locked Deck Lists. Remove
  any legacy completed-replay draft and old event history without otherwise
  resetting its waiting-room configuration.
- Reset Active and Finished rooms to Waiting, clear `gameInstanceId`, mark all
  members `ready: false` and `connected: false`, and delete their Game Record,
  `lastCompletedReplayDraft`, Locked Deck Lists, `event:*`, Command receipts,
  trusted-randomness receipts, and resolution transactions.
- Start each surviving non-dissolved room's replacement event log with one
  `LegacyGamePurged { epoch }` event and show the one-time message
  `系統版本更新，上一局已清除，請重新準備。` A repeated call for the same
  epoch must not duplicate the event or reset a newly prepared room.
- Add a protected ReplayArchive management operation such as
  `purgeLegacyArchive(epoch)` that calls Durable Object
  `storage.deleteAll()`. Calling it again must remain safe. Do not replace or
  retire the namespace, because new-version Replays continue using it.
- Add `apps/web/scripts/purge-legacy-games.ts` and a package script. Require
  exactly one explicit `--env staging|production`; default to dry-run and
  require `--confirm` for mutation. Require a stable purge epoch, accept secrets
  only from environment variables, and never print secret values.
- Protect management endpoints with a dedicated secret and make them available
  only while an explicit deployment/configuration maintenance gate is active.
  This gate is configuration, not persisted schema.
- During maintenance, reject new game commands and Replay creation. Enter
  maintenance before enumeration and leave it enabled after any failure.
- Use D1 room data plus the Cloudflare Durable Objects Namespaces/Objects API to
  enumerate GameRoom and ReplayArchive objects, including storage with no
  surviving D1 replay reference. Follow API cursors until exhausted.
- Delete only rows from `player_saved_replay` and
  `replay_archive_lifecycle`; preserve authentication, Player profiles, custom
  Deck Lists, and public room index data. Use a D1 atomic batch/transaction for
  the replay-table deletion.
- Verification must prove: every incompatible Game Record is absent; active and
  finished rooms are Waiting; preserved room count and identities match the
  dry-run inventory; both D1 replay tables are empty; every enumerated legacy
  ReplayArchive has no stored archive; sampled old Replay IDs return 404; and
  a second run reports no further mutation.
- Reopen traffic only after all verification succeeds. Document the exact
  staging-first and production command sequence, but never invoke the remote
  purge automatically during deploy, tests, or implementation.

Cloudflare references for the management implementation:

- [Access Durable Object storage](https://developers.cloudflare.com/durable-objects/best-practices/access-durable-objects-storage/)
- [List Durable Object namespace objects](https://developers.cloudflare.com/api/resources/durable_objects/subresources/namespaces/subresources/objects/methods/list/)

## Testing Decisions

- Add Rust tests for arbitrary submission order, both capacities, already-chosen
  validation, private Card projection, the completion barrier, exactly one
  first shuffle request, Turn Order shuffle/deal, and direct-versus-replay state
  equivalence.
- Add serialization tests for the changed canonical events and exact camelCase
  public DTO fields.
- Add Online Command Transaction and Durable Object tests for near-simultaneous
  choices, same-ID retry, different-ID duplicate, sequence conflicts, and
  exactly-once transition into trusted randomness.
- Add Web tests for public remaining/completed progress, both Players' initial
  chooser availability, immediate Card and random-shortcut submission, owner
  reconnect knowledge, other-viewer Card Back, duplicate-tab reconciliation,
  and stale same-Game-Instance response rejection.
- Add representative E2E coverage proving both Players initially see the
  chooser, the first submitter waits, and the final submitter releases shuffle
  and initial deal.
- Add CLI tests for argument validation, dry-run default, secret redaction,
  cursor pagination, room preservation, status reset, selective D1 deletion,
  ReplayArchive `deleteAll()`, same-epoch rerun, partial failure, maintenance
  fail-closed behavior, and verification failures.
- Validate with focused Rust and Web tests, full `cargo test`, Web unit and
  integration tests, type checking, relevant Playwright coverage, formatting,
  and diff checks.

## Out Of Scope

- Supporting or migrating an in-progress legacy Game Record.
- Preserving, converting, or replaying a legacy completed Replay.
- Running the purge as a schema migration, Durable Object class migration,
  ordinary deployment hook, or automatic startup task.
- Adding a global room revision or redesigning all room-state delivery.
- Changing Pending Choice, generic timeout, auto-answer, forfeit, cancellation,
  or Pouch trigger semantics after Game Preparation.
- Reordering canonical events into Turn Order after accepted commits.

## Governing Context

- ADR-0020 owns Initial Pouch Selection's Rules Engine and Game Preparation
  boundary.
- ADR-0026 keeps Initial Pouch Selection distinct from Pending Choice.
- ADR-0014 owns trusted randomness and deterministic replay.
- Completed issue #73 owns Online Command Transaction identity, receipts,
  sequence validation, atomic commits, and post-commit delivery boundaries.
- `CONTEXT.md`, `docs/rule.md`, and decision 41 define the confirmed semantics.
- The management CLI must follow current Cloudflare Durable Object and D1 APIs;
  platform-specific behavior must be verified against official documentation
  during implementation.

## Blocked by

- none
