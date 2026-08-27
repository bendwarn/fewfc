# 35 Build synchronized online game rooms

## Triage

ready-for-agent

## What to build

As a player, I want to create or join a public or private online room, synchronize room and game state with the other players, and complete a match without manually refreshing the page or advancing engine phases.

Public and private rooms differ only in how players discover and enter them. After a match starts, both use the same gameplay flow.

Use a Cloudflare Worker as the authenticated gateway and route each room to an authoritative Durable Object. Use WebSockets to push viewer-filtered room state, Public Game State, and Public Event Feed updates to the Nuxt client. Nuxt state management consumes these updates; it is not the cross-client synchronization mechanism.

## Acceptance criteria

- [x] A player can create either a public or private room for two or four players.
- [x] Visibility and player count are immutable after room creation.
- [x] Public rooms appear in the public list only while they can still be joined.
- [x] Private rooms do not appear publicly and provide both an invitation link and a short room code.
- [x] A player can belong to multiple waiting or active rooms, all shown under "My rooms".
- [x] Guests can create and join online rooms and recover their session in the same browser; cross-device recovery requires an account.
- [x] Non-host players can toggle ready and unready before the match starts.
- [x] A waiting-room disconnect clears a non-host player's ready state.
- [x] The host can start only when the room is full and every other player is connected and ready.
- [x] A host disconnect does not close, expire, or otherwise change the room.
- [x] Navigating away from a room does not leave or dissolve it.
- [x] A non-host player can use `Leave` to immediately leave a waiting room without confirmation.
- [x] The host can use `Remove` to immediately remove another player from a waiting room.
- [x] The host can use `Dissolve` to immediately close a waiting room without confirmation and notify its members.
- [x] `Leave`, `Remove`, and `Dissolve` are unavailable after a match starts.
- [x] Starting a two-player match randomizes first player.
- [x] Starting a four-player match randomly creates two equal teams and a legal alternating turn order.
- [x] Random setup results are recorded once and remain stable across reconnect and replay.
- [x] Team and turn order are shown briefly before the engine advances to the first player decision, without another confirmation.
- [x] The server automatically advances non-decision phases and stops only at an actual player decision.
- [x] General-player screens do not expose manual refresh, manual phase advancement, or other debug controls.
- [x] Waiting and active room views have no separate status bar or central player strip: Back sits at the battlefield's top-left, waiting-room details stay in its overlay, action status appears only in the acting player's area, and turn count appears only in the match record.
- [x] A two-player battlefield keeps player areas at the top and bottom; a four-player battlefield keeps the viewer at the bottom, their teammate opposite, and the two opponents at the left and right.
- [x] On mobile, four-player side areas use a compact vertical layout with avatar, name, HP, connection, action status, and hand count; only top and bottom areas render card rows.
- [x] Each player area shows a compact connected/disconnected indicator; only the viewer's area adds reconnecting text, and no standalone synchronization label remains.
- [x] Hidden hands and the deck use the same text-free default Card Back while retaining an accessible "牌背" label.
- [x] The central Formation area shows only the Formation used during the Previous Turn and is empty when that turn used no Formation; it shows the Formation name and used cards only to the extent allowed by the viewer's Public View.
- [x] A player selects hand cards first, after which the UI lists matching formation names only.
- [x] The Formation area keeps the Previous Turn's Formation visible above current Formation candidates; candidate details appear in a non-resizing hover, focus, or long-press overlay.
- [x] The separate action panel is removed: selection count stays by the viewer's hand, while Formation candidates and `Skip` share the central candidate area.
- [x] Formation submission progress and rejection reasons replace the candidate controls in place; rejection restores the candidates without clearing the selected cards.
- [x] Long press reveals formation details on touch devices; hover and focus provide the desktop equivalent.
- [x] Selecting a formation activates it immediately unless target or effect choices are required.
- [x] Before the final Command is submitted, a player can return from target or effect selection without losing the selected cards or formation candidates.
- [x] A normal Action Command implicitly declines any remaining optional active-effect Commands.
- [x] `Skip` is shown only when `PassAction` is legal and at least one optional activatable effect can be declined.
- [x] When `PassAction` is legal and no optional activatable effect exists, the server passes automatically.
- [x] The UI does not model "no formation can activate" as a normal state; a formation may instead resolve as invalidated or no effect according to the rules.
- [x] While awaiting server acknowledgement, the UI shows a processing state and prevents duplicate submission without optimistic game-state updates.
- [x] A rejected Command preserves the player's local selection where possible and displays the rejection reason.
- [x] The game screen always contains a viewer-filtered match record area.
- [x] Every visible Public Event Feed entry has a plain-language Chinese title and description; internal event identifiers such as `DeckPrepared` are never rendered or used as display fallbacks.
- [x] Deck preparation, shuffle, and initial deal details are consolidated into one player-facing "對局開始" entry while the canonical Game Record remains complete.
- [x] Desktop shows a full match-record sidebar; mobile shows the latest two or three entries and can expand the full record.
- [x] Hidden hands, covered cards, private choices, and private record details are filtered on the server rather than hidden by the client.
- [x] Teammates can see each other's hand count but not card faces.
- [x] An in-match disconnect does not pause the match or cause a timeout loss; other players see the disconnected state.
- [x] While reconnecting, the player can inspect the last state but cannot submit Commands.
- [x] Reconnection replaces the client with the latest authoritative state without special recovery of unsubmitted local selections.
- [x] Starting another joined room or reaching a player's turn in a background match produces a non-blocking global notification with an action to open that room.
- [x] Starting one match does not cancel the player's membership or ready state in other rooms.
- [x] Notifications are transient and are neither persisted nor replayed; "My rooms" is the authority after reconnect.
  Browser retention is superseded by [local issue 77](077-retain-and-surface-player-notifications.md):
  ordinary received notifications are retained locally for room-list dots;
  server delivery still has no persistence or replay.
- [x] Match completion shows the result and returns players to the same room.
- [x] Returning to the room after a match resets every non-host player to unready before another match can start.

## Technical constraints

- [x] Use a room-scoped Durable Object as the single authority for room membership, Commands, Game Record, and per-viewer updates.
- [x] Use WebSocket Hibernation so idle room connections can remain open without requiring polling.
- [x] Maintain one player-scoped global notification WebSocket plus one WebSocket for the currently open room.
- [x] Do not send canonical Game State or canonical Game Events to browser clients.
- [x] Server-side start validation is atomic so membership, connectivity, readiness, and capacity cannot race.

## Non-goals

- Beginner or learning-mode lists of every currently available action.
- Spectating.
- Browser or operating-system notifications.
- Turn timers, room expiry, timeout losses, or automatic room cleanup.
- Surrender, match cancellation, or abnormal-match adjudication.
- Leaving, removing players, or dissolving a room during an active match.
- Manual team selection, seat selection, or room-setting changes.
- Card Back customization beyond keeping the default appearance replaceable.

## Blocked by

- None - can start immediately
