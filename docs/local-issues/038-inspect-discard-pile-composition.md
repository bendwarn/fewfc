# 38 Inspect discard pile composition

## Triage

ready-for-agent

## What to build

As a player, I want to select the discard pile and inspect its composition so I can
see how many Card Instances of each discarded Card Definition are currently public.

The discard pile is already fully visible through the Public State View. The client
groups its Public Cards by Card Definition, where one kind means one element and
level combination, and derives counts without adding canonical or hidden game data
to the API.

## Acceptance criteria

- [x] Selecting the discard pile opens a composition overlay.
- [x] Discarded cards are grouped by element and level into 25 Card Definition
      counts.
- [x] The overlay always renders all 25 Card Definitions, including zero counts, so
      its dimensions and item positions remain stable as the discard changes.
- [x] The fixed grid uses element columns in the order metal, wood, water, fire,
      earth and level rows from 1 through 5.
- [x] Column headers display only the element names, row headers display levels 1
      through 5, and each data cell displays only its count.
- [x] Zero-count cells remain present with lower visual emphasis.
- [x] Desktop anchors the composition popover above the discard pile.
- [x] Mobile presents the same fixed grid in a centered modal that stays within the
      viewport.
- [x] While the overlay is open, the next selection anywhere on the screen,
      including inside the overlay, closes it and returns focus to the discard-pile
      trigger.
- [x] Pressing Escape closes the overlay and returns focus to the discard-pile
      trigger.
- [x] The read-only overlay has no separate close icon.
- [x] A mandatory action-choice overlay takes precedence and prevents opening the
      discard composition as a second stacked overlay.
- [x] Outside mandatory action choices, every player may inspect a non-empty
      discard pile regardless of whose turn it is.
- [x] After the game ends, the result stays outside the battlefield and every
      player may inspect the final discard pile directly from the table.
- [x] When the discard pile is empty, the trigger displays zero but is disabled and
      does not open the overlay.
- [x] While the overlay is open, its counts update immediately when the Public
      State View changes.
- [x] If a state update empties the discard pile, the overlay closes, focus returns
      to the now-disabled trigger, and the trigger continues to display zero.
- [x] Each count represents the number of matching Card Instances currently in the
      discard pile.
- [x] Counts are derived only from the viewer's Public State View.
- [x] The existing discard-pile total remains visible on the battlefield.
- [x] The trigger and grid expose complete accessible labels, including the discard
      total and each cell's element, level, and count.
- [x] Automated tests cover grouping, all 25 stable cells, opening and closing
      behavior, live updates, automatic closure when emptied, and the disabled
      empty state.

## Validation

- `bun run test`
- `bun run test:e2e`
- `bun run typecheck`
- `cargo test`
- Production Nuxt and rules WASM build
- Playwright E2E through Brave at 377 x 734 and 1280 x 720, including accessible
  table structure, fixed cell count, responsive placement, all close paths,
  mandatory choice precedence, WebSocket count updates, and automatic closure
  when the discard is emptied

## Non-goals

- Showing deck composition or hidden hands.
- Persisting the overlay across reload or route changes.
- Adding discard composition to canonical game state or the Game Record.
- Adding a separate discard action to the result panel.

## Blocked by

- None - can start immediately
