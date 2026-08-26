---
status: accepted
---

# Keep Room Observers outside started-game membership

Public Online Game Rooms remain discoverable while waiting or playing, even
when every Player Seat is occupied. Full waiting rooms and started Games admit
Room Observers; finished Games are not a new live-observation entry point.
Room Observers occupy no Player Seat, make no Player Decisions, and see only
public Game information. Live
observation does not inherit Replay Omniscience or another Player's private
perspective. This revisits the discovery restriction and spectating non-goal in
[issue 035](../local-issues/035-build-synchronized-online-game-rooms.md).

## Admission and capacity

This feature introduces no separate product limit on the number of Room
Observers. They do not consume any of the room's two or four Player Seats;
Player and Room Observer counts are presented separately. Any future
observation-capacity limit is a separate decision informed by actual load.

Private rooms use the same observation and Seat Promotion rules, including
admitting Room Observers when full or after the Game starts. They remain absent
from public discovery. A new member must supply a valid room code or invitation
link under the existing private-room admission policy; observation does not
grant access merely from knowing a room's identity.

Joining a waiting room with an available Player Seat makes the newcomer a
Player; joining a full waiting room or an already-started Game makes them a
Room Observer. This role is assigned automatically, without a separate
observe-only mode or an opt-out from Seat Promotion. The observation entry
explicitly tells newcomers that a pre-start vacancy may automatically promote
them to Player in join order.

## Seat Promotion and connectivity

Before a Game starts, a departing Player's vacant seat automatically goes to
the first connected Room Observer in the Observer Queue.
Explicitly leaving loses that Observer's position; rejoining places them at
the end. Once the room starts, including Game Preparation, Player membership
remains fixed and Room Observers cannot replace Players. The start boundary
continues to follow
[ADR 0020](0020-keep-initial-pouch-selection-in-the-rules-engine.md).

Seat Promotion leaves the new Player unready. They must become ready themselves
and use their own Deck List, with the Locked Deck List captured according to
[ADR 0004](0004-lock-player-decks-when-ready.md); neither readiness nor a Deck
List is inherited from the departing Player. Notify the promoted member with
`已補為玩家，請準備` through the existing in-app notification mechanism, not
a separate inline hint. The Seat Promotion notification must remain visible
even when the recipient is currently viewing that room; the usual suppression
of current-room notifications must not hide it.

Disconnecting does not end room membership or release an occupied Player Seat;
an explicit departure or owner removal releases that seat. A disconnected Room
Observer retains their original join position but is skipped when choosing
someone for Seat Promotion, so the next connected Room Observer can fill the
vacancy without waiting for the disconnected Observer.

When a Room Observer reconnects to a waiting room that still has vacant Player
Seats, Seat Promotion immediately fills those seats from the currently
connected Room Observers in their original join order. It does not require
another Player departure. Reconnection never displaces someone already
promoted into a seat, and a disconnected Observer does not reserve a vacancy.

An existing eligible Room Observer has priority over a newcomer for a vacant
Player Seat. Joining, leaving, reconnecting, and starting a Game must preserve
the same Seat Promotion and membership boundaries when they occur together.

## Observation does not advance Game history

Room Observer admission, departure, removal, connection changes, queue
maintenance, and pre-start Seat Promotion are room-membership changes, not
Game events. Persist their membership and present notifications without
appending to the room/command event log or advancing `nextSequence`,
`GameRecord.sequence`, or the Rules Engine record. After Seat Promotion,
the member's subsequent Player operations follow the ordinary Player path.

An Observer may enter, disconnect, reconnect, or leave while a Player is
acting without changing that Game's version or invalidating its command
checkpoint. Do not make Observer actions advance the Game Record merely to
keep its sequence aligned with room events; keep those actions outside the
sequence entirely. This preserves the command concurrency check while
keeping live observation independent of play and Replay history.

## Room lifecycle

The room owner retains the existing lifecycle: they cannot leave independently
or transfer ownership through Seat Promotion. They can dissolve the waiting
room, which closes it for Players and Room Observers alike without triggering
Seat Promotion. Dissolution remains unavailable after the Game starts.

Room Observers may explicitly leave during any room phase. The owner can remove
a Room Observer only in the waiting room; removal after the Game starts is not
available. This is a product-scope decision because this release does not add
an in-game room-management interface, rather than a requirement of the Rules
Engine. Server authorization must enforce the same restriction. Leaving or
being removed ends the Observer's membership and loses their queue position.

Game completion and the subsequent return to the waiting room preserve the
existing Player Seats, Room Observer memberships, and Observer join order.
Existing waiting-room readiness reset rules still apply. Completing a Game
does not release Player Seats or automatically rotate Room Observers into the
next Game; they continue waiting for a vacant seat.

## Interface

- Room lists show Player occupancy and the Room Observer count separately.
  The entry action is labeled `觀戰` for full waiting rooms and started Games.
- `我的房間` includes rooms joined as a Room Observer and marks that role as
  `觀戰`. Back navigation and disconnection do not remove these memberships.
- The waiting room has a separate Room Observer list in Observer Queue order,
  with each Observer's connection state. The owner removes Room Observers from
  this list; the active-game screen has no owner-removal controls.
- A Room Observer has an explicit `離開觀戰` action that ends membership and
  loses their queue position. The existing top-left `返回` remains navigation
  only and preserves membership and queue position.

## Preserved authority and rationale

Room Observer membership does not grant the Players' authority to reset a
finished Game or save its Replay. Existing explicit Replay sharing remains a
separate policy; live observation does not change it. Owner removal retains
its existing meaning as departure rather than an account ban, so a subsequent
admission follows the ordinary entry rules and starts a new queue position.

Mid-game replacement was rejected because taking over a Player would transfer
their private information and control of an identity already committed to the
Game Record. Keeping Seat Promotion before that boundary supports automatic
waiting-room backfill without introducing takeover rules or changing the
meaning of historical Player decisions.
