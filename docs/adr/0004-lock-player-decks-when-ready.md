---
status: accepted
---

# Lock player deck lists when ready

The Web application stores one current custom Deck List per account. In a
Personal Deck room, the room captures a validated Locked Deck List when a
non-owner becomes ready and when the owner starts the game, automatically using
the Preconstructed Deck List when the account has no valid custom list. The
snapshot records the name and exact contents actually selected after fallback.
Game Setup uses only these room snapshots.

Changing enabled Rule Modules, cancelling readiness, or leaving the room
invalidates affected waiting-room snapshots. Started-game snapshots are
immutable.

Reading live account data when a game starts was rejected because edits after
readiness could silently change the agreed setup, make readiness misleading, and
couple deterministic game creation to mutable profile state. Multiple saved deck
slots remain a separate future capability.
