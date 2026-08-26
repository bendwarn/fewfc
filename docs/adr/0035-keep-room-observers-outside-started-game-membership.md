---
status: accepted
---

# Keep Room Observers outside started-game membership

Public Online Game Rooms remain discoverable and admit Room Observers while
waiting or playing, even when every Player Seat is occupied; finished Games
are not a new live-observation entry point. Room Observers occupy no Player
Seat, make no Player Decisions, and see only public Game information. Live
observation does not inherit Replay Omniscience or another Player's private
perspective. This revisits the discovery restriction and spectating non-goal in
[issue 035](../local-issues/035-build-synchronized-online-game-rooms.md).

Joining a waiting room with an available Player Seat makes the newcomer a
Player; joining a full waiting room or an already-started Game makes them a
Room Observer. This role is assigned automatically, without a separate
observe-only mode or an opt-out from Seat Promotion. The observation entry
explicitly tells newcomers that a pre-start vacancy may automatically promote
them to Player in join order.

Before a Game starts, a departing Player's vacant seat automatically goes to
the earliest-joined Room Observer who remains in the room and is connected.
Explicitly leaving loses that Observer's position; rejoining places them at
the end. Once the room starts, including Game Preparation, Player membership
remains fixed and Room Observers cannot replace Players. The start boundary
continues to follow
[ADR 0020](0020-keep-initial-pouch-selection-in-the-rules-engine.md).

Seat Promotion leaves the new Player unready. They must become ready themselves
and use their own Deck List, with the Locked Deck List captured according to
[ADR 0004](0004-lock-player-decks-when-ready.md); neither readiness nor a Deck
List is inherited from the departing Player. Notify the promoted member with
`已補為玩家，請準備` through a notification, not a separate inline hint.

Disconnecting does not end room membership or release an occupied Player Seat;
an explicit departure or owner removal releases that seat. A disconnected Room
Observer retains their original join position but is skipped when choosing
someone for Seat Promotion, so the next connected Room Observer can fill the
vacancy without waiting for the disconnected Observer.

Mid-game replacement was rejected because taking over a Player would transfer
their private information and control of an identity already committed to the
Game Record. Keeping Seat Promotion before that boundary supports automatic
waiting-room backfill without introducing takeover rules or changing the
meaning of historical Player decisions. The remaining room-management details
are still being designed separately.
