# 37 Route and restore player navigation

## Triage

ready-for-agent

## What to build

As a player, I want each major application page to have a stable Nuxt route so
refreshing or reopening the URL restores the page and authoritative room state I was
viewing instead of returning to the login screen.

The URL identifies the application page and room, not transient game phases,
selections, or connection state. Waiting, active, and completed views of one room
share one route and restore their current state from the server.

## Acceptance criteria

- [x] `/login` renders login and registration.
- [x] `/rooms` renders the room lobby.
- [x] `/rooms/:gameId` renders the referenced room in its current waiting, active, or
      completed state.
- [x] `/` redirects authenticated users to `/rooms` and unauthenticated users to
      `/login`.
- [x] Refreshing `/rooms` keeps the player in the lobby and reloads room lists.
- [x] Refreshing `/rooms/:gameId` reloads the authoritative viewer-filtered room
      state and reconnects room synchronization.
- [x] Private invitation links use `/rooms/:gameId?invite=:token` rather than the
      legacy `?room=` query.
- [x] Game phase, pending local selections, and connection state are not encoded in
      the route.
- [x] An unauthenticated request for a protected route redirects to
      `/login?redirect=<internal path>`.
- [x] Successful account or guest authentication returns the player to the preserved
      route.
- [x] Redirect targets accept only local application paths and cannot redirect to an
      external origin.
- [x] Opening `/rooms/:gameId` as a current member loads the room without changing
      membership.
- [x] Opening a joinable public room route as an authenticated non-member
      automatically joins that room; private rooms additionally require a valid
      invitation credential.
- [x] If the room cannot be joined because it is full, active, dissolved, or unknown,
      the route remains visible and renders a clear result without changing
      membership.
- [x] Creating, joining, or opening a room from the lobby pushes
      `/rooms/:gameId` so browser Back returns to the lobby.
- [x] The room Back control navigates to `/rooms` without leaving room membership.
- [x] Leaving, dissolving, or being removed from a room replaces the current history
      entry with `/rooms` so Back does not reopen the invalid room.
- [x] Starting and finishing a match do not change the room URL.
- [x] While session restoration is pending, protected routes show a dedicated loading
      state rather than briefly rendering login or lobby content.
- [x] A room route shows a room-loading state until authoritative room data is
      available and does not render a fixture or empty battlefield.
- [x] The waiting room or battlefield renders only after the authoritative response;
      WebSocket reconnection then resumes live synchronization.
- [x] Lobby tabs are route state represented by `/rooms?tab=create` and
      `/rooms?tab=join`.
- [x] Unsubmitted room forms, card and formation selections, detail overlays, and
      record expansion are local UI state and reset on reload.
- [x] An inaccessible room route renders a stable result page rather than
      automatically redirecting.
- [x] Full rooms show `房間已滿`; active rooms opened by non-members show
      `對局已開始`; unknown and dissolved rooms show `找不到這個房間`.
- [x] The result page offers only a `返回房間大廳` action.
- [x] A private room's plain `/rooms/:gameId` route reveals no room existence or
      status to a non-member and renders `找不到這個房間`.
- [x] A valid invitation token or manually submitted room code authorizes an
      authenticated non-member to join a private waiting room.
- [x] After joining, the client replaces the invitation URL with
      `/rooms/:gameId`, removing the token from browser history while membership
      becomes the authority for future reloads.
- [x] Private rooms issue separate high-entropy invitation-link tokens and
      human-entered short room codes.
- [x] Both invitation credentials are valid only while the room is waiting and has
      capacity, and become invalid when the match starts or the room dissolves.
- [x] Hosts cannot revoke or regenerate invitation credentials in this version.
- [x] Intentional logout replaces the current route with `/login`, omits a redirect
      target, disconnects protected subscriptions, and prevents Back from revealing
      the previous player's room view.
- [x] Only an expired or missing session encountered on a protected route preserves
      that internal route as the post-login destination.
- [x] Opening another joined room from a notification pushes its route so browser
      Back returns to the previous room.
- [x] A room route transition disconnects the previous room WebSocket, fetches the
      destination room's authoritative state, and then connects its room WebSocket.
- [x] Browser Back to a prior room performs the same authoritative reload and does
      not reuse a stale room snapshot.
- [x] The player-scoped notification WebSocket remains connected across authenticated
      page and room route transitions.
- [x] Browser tests cover refresh restoration for `/login`, `/rooms`, and waiting,
      active, and completed room states.
- [x] Browser tests cover unauthenticated invitation return, token removal, and
      private-room non-disclosure.
- [x] Browser tests cover push/replace history for create, join, Back, leave,
      dissolve, and removal flows.
- [x] Browser tests cover room A-to-B navigation and Back with exactly one active room
      WebSocket and a fresh authoritative snapshot at each destination.

## Technical constraints

- [x] Replace the `screen` state machine in `app.vue` with Nuxt file-based pages and
      `<NuxtPage>`.
- [x] Implement shared session-aware route middleware for protected pages.
- [x] Keep authenticated session and global notifications above page lifetimes.
- [x] Scope room state and the room WebSocket to the active room route and dispose
      them when that route changes.

## Non-goals

- Persisting unsubmitted card, formation, target, or effect selections across reload.
- Encoding turn or engine phase in the URL.
- Restoring non-authoritative client state.

## Blocked by

- None - can start immediately
