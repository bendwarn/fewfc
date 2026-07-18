---
status: accepted
---

# Let Nuxt route modules own screen lifecycles

Nuxt routes are the only source of screen identity. The root application module
only composes layouts, pages, and route announcements; it does not derive a
screen enum from paths or orchestrate route-specific loading. Auth middleware
owns navigation policy, the default and auth layouts own their distinct shells,
and every user-reachable URL has a real page module.

Each page module owns its view, loading, errors, ephemeral state, focus, keyboard
behavior, and route lifecycle. The Game Room session is scoped to the Game page
and reloads authoritative room state after navigation, while Player Notifications
remain in the authenticated layout. Rules Catalog is a shared cached module
because Deck, Lobby, and Game pages are real callers. Leaving a route discards
unsubmitted page state rather than promoting it into the global shell.

Existing URLs, invite and replay query semantics, observable UI behavior, and
server-owned state remain unchanged. CSS moves with module ownership without a
visual redesign. Live-game and replay Public State presentation are not unified
by this decision, and large page modules are not split merely to reduce file
length. Implementation is sequenced after local issue 70 because Pending Choice
deepening first removes overlapping interaction knowledge from the current root
application module.

## Considered options

- Keeping one root application orchestrator was rejected because the page
  modules were pass-through adapters and route knowledge, data loading, focus,
  and errors had no locality.
- Creating one composable for every page was rejected because single-caller
  pass-through modules would add interface without depth. A new seam requires
  independent behavior, multiple real callers, or a positive deletion test.
- Passing a complete room response from Lobby to Game was rejected because it
  couples page lifecycles and can be stale by the time navigation completes.
- Persisting unsaved page drafts across navigation was rejected because local UI
  state is neither canonical nor shared state.

## Consequences

Route middleware tests authentication and safe redirects; pure module tests cover
navigation and Rules Catalog policy; TypeScript type checking protects page and
layout contracts; and Brave Playwright flows cover deep links, navigation,
route-local loading, focus, and session teardown. Tests that read the root
application source as text are replaced with observable route or DOM assertions.
