# 77 Retain and surface Player Notifications

## Triage

ready-for-agent

## Goal

Keep a room member aware of received Player Notifications through room-list
dots after their floating prompts disappear, while keeping one notification per
Online Game Room.

## Confirmed Decisions

The following directions were settled during grill-with-docs and handed to a
Terra subagent for implementation at the user's request. The final simplified
clearing behavior below follows the recommendation at that handoff.

- Automatically hide a floating notification after five seconds without
  deleting its retained notification, except for the transient-only removal
  and dissolution notices described below.
- Retain received notifications in browser `localStorage`, rather than adding
  a server notification store.
- Keep at most one notification per room, not a chronological event history.
- Do not add a notification list, notification page, or new profile page.
- Mark rooms with notifications using a dot in the room list.
- Leave the header connection status and its interaction unchanged. This
  supersedes the earlier proposal to open a notification list from that control.
- A floating prompt's close control only hides that prompt; it does not consume
  the retained notification or clear the room dot.
- Successfully entering a room consumes that room's retained notification and
  clears its dot.
- Retain entries for seven days, with at most 100 distinct rooms; exceeding
  that cap removes the oldest entries. Replace the current eight-room cap.
- Store notifications separately by user ID. Logout does not delete the stored
  entries; the same account can restore them in the same browser, and another
  account must not display them.
- A notice that the room was dissolved or the recipient was removed does not
  offer an action to enter that room.
- Do not introduce a manual-clearing control. Ordinary entries remain until
  successful room entry, seven-day expiry, or eviction by the 100-room cap.
- Removal and dissolution notices appear as five-second prompts only and are
  not retained. Receiving either also clears any older retained notification
  for that room.

## Storage Boundary

The existing server delivers notifications only to connected clients and does
not retain or replay them. Local storage can retain messages this browser
received; it cannot retrieve messages missed while offline or synchronize
notifications across devices. Clearing browser storage removes those retained
notifications.

Local issue 35 originally specified notifications that were neither persisted
nor replayed. This follow-up changes browser retention only; it does not add a
server inbox or make notifications authoritative room state. The room API and
My rooms remain authoritative after reconnect.

## Existing Constraints To Preserve

- Player Notifications remain owned above route lifetimes by the authenticated
  layout, as established by ADR 0027.
- Room-state invalidation messages (`roomsChanged`) are not Player
  Notifications and do not become list entries.
- ADR 0035 requires Seat Promotion to be shown even inside the affected room.
  An ordinary same-room update must not immediately replace that notice.
- A retained notification is not authority for the current turn, room
  membership, or permission to enter a room.
- Restoring retained notifications must not replay live-delivery side effects,
  such as navigating away because of an old removal notification.
- Floating-prompt visibility, retained notification data, and authoritative
  room state must not be conflated. Today one array also drives lobby attention
  labels and removal navigation, so adding a timeout to the current dismiss
  function would lose more than the floating prompt.

## Acceptance Criteria

- [x] A newly received ordinary notification shows a five-second floating
  prompt and leaves a dot on the matching room after that prompt closes.
- [x] Closing the prompt manually hides only the prompt.
- [x] Refreshing restores retained room dots without showing old prompts or
  replaying live-delivery navigation effects.
- [x] Each room has at most one retained notification; Seat Promotion keeps
  its existing protection against an immediate ordinary room update.
- [x] Successfully loading a room consumes its retained notification; failed
  navigation or loading does not consume it.
- [x] Seven-day expiry and the 100-room cap bound retained notifications.
- [x] Logout preserves the current account's stored notifications; another
  account cannot see them, and the original account can restore them.
- [x] Removal and dissolution show a transient reason without an enter-room
  action, clear older retained entries for that room, and are not persisted.
- [x] The room-list dot is accessible and does not claim that the notification
  itself proves it is currently that Player's turn.
- [x] The connection-status control and existing navigation remain unchanged;
  no notification list, profile page, or manual-clearing control is added.

## Implementation And Verification

Completed by a Terra subagent with independent parent-agent review and Brave
browser verification. The authenticated layout retains notification ownership;
the composable separates retained notifications from live floating prompts.
Storage validation, account changes, failed storage writes, expiry timers, and
delayed cross-tab storage events are covered by focused tests. Restoring saved
entries never restores floating prompts or live removal navigation.

The room-list action remains `進入`; a notification dot does not assert that it
is still the recipient's turn. Removal and dissolution keep their five-second
reason visible after returning to the lobby without retaining those entries.

Verified on 2026-08-27:

- `cargo test --quiet`: passed.
- `bun run test:unit`: 137 tests passed across 41 files.
- `bun run test:component tests/component/LobbyRoomList.nuxt.spec.ts tests/component/PlayerNotifications.nuxt.spec.ts`:
  8 tests passed across 2 files.
- `bun run typecheck`: passed.
- `PLAYWRIGHT_REUSE_SERVER=0 bun run test:e2e tests/e2e/player-notifications.spec.ts --reporter=line`:
  3 Brave tests passed against a fresh build and isolated local Worker storage.
  Coverage includes five-second expiry, manual prompt closing, reload, failed
  room entry preserving the dot, successful entry consuming it, and transient
  removal/dissolution navigation and persistence.
- `bun run test:e2e:built tests/e2e/room-observer-lifecycle.spec.ts --grep 'back navigation' --reporter=line`:
  the existing Brave Seat Promotion and reconnect regression passed against the
  same build with fresh local Worker storage.
- `git diff --check`: passed.

No server notification persistence, notification list, deployment, or canonical
Game Record change was introduced.

## Related Documentation

- `CONTEXT.md`: Player Notification and Online Game Room.
- Local issue 35: original transient notification contract.
- ADR 0027: authenticated-layout ownership of notifications.
- ADR 0035: Seat Promotion visibility and separation from Game history.
