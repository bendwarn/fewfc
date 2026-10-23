# Storybook

Component stories live in `stories/components/`; deterministic shared view data
lives in `stories/fixtures/`. The preview loads the app's Tailwind stylesheet and
starts in the light theme. `test:storybook` runs a Chromium smoke test for every
story, and stories with `play` functions also exercise browser interactions.

```sh
cd apps/web
bun run storybook       # http://localhost:6006
bun run test:storybook  # Starts Storybook and runs every story in Chromium
bun run build:storybook
```

`@storybook/addon-mcp` exposes the Storybook MCP endpoint at
`http://localhost:6006/mcp` while the development server is running. The
component manifest is enabled for component and story discovery.

## Validation scope

The acceptance policy is defined in
[Web App Agent Instructions](../AGENTS.md#ui-browser-and-e2e-validation).
Choose the test layer by the contract it exercises:

| Layer | Coverage | Examples |
| --- | --- | --- |
| Storybook visual acceptance | Component, composed-screen, and page appearance | Themes, mobile layout, long labels, two- and four-Player layouts, observer and replay views |
| Storybook interaction tests or Playwright assertions against stories | UI behavior with supplied data and dependencies | Selection, disabled controls, emitted events, error messages, keyboard input, focus, and element geometry |
| Unit tests | Pure logic and isolated module contracts | Draft composition, display mappings, and shared/Worker helpers covered by `test:unit` |
| Nuxt and service integration tests | Contracts requiring the application or server context | Composables, real router/middleware wiring, HTTP/DTO parsing, and service coordination |
| Playwright E2E | Representative flows across real service boundaries | Login and room entry, command-to-Rust-to-UI updates, multiple sessions receiving WebSocket updates, reconnect, and private-hand filtering |
| Rust tests | Authoritative rules behavior | Command validation, canonical events, replay, and hidden-information projection |

A complete page rendered with mocked APIs establishes the UI's response to those
inputs. Evidence for actual authentication, authoritative Commands, transport,
and server-side filtering comes from integration tests and representative E2E
flows. Running Playwright against a story is a UI test; it exercises only the
dependencies that story actually connects.

## Visual acceptance

Create page or composed-component stories for changes to a complete screen.
Include the viewport sizes, themes, and public-data perspectives required by the
design. Leaf-component stories alone leave the surrounding page composition
unverified. For the battle-screen acceptance cases, see
[Battlefield UI Design](../../../docs/ui-battlefield-design.md#後續實作的驗收重點).

Manual review in Storybook can accept appearance and layout. Automated screenshot
comparisons can detect unintended visual changes once a visual runner and
reviewed baselines are configured. Review intentional differences against the
design before updating a baseline.

The current `test:storybook` command checks rendering and `play` assertions; it
does not perform screenshot comparisons. The current stories cover components;
page-level visual acceptance requires adding the corresponding composed-screen
or page stories. The preview's initial light theme also does not establish dark
theme coverage without a story or review that explicitly selects that theme.

## Coverage and migration

Stories for API-backed competition controls currently begin in safe,
unauthenticated states. Add deterministic authenticated, loading, success, and
error scenarios when migrating the corresponding UI claims. Reuse public DTO
fixtures and rendering code, keeping authoritative rule decisions in Rust.

Inventory existing assertion claims before moving tests. Migrate UI states,
isolated parent-child interactions, and presentation assertions into stories,
and remove an old case only after its replacement passes. Keep Nuxt-specific
composable, router, transport, and application integration contracts in their
appropriate unit or integration suites. Retain representative E2E flows for real
service wiring rather than enumerating every UI or rules variant through them.

## Full page dependency fixtures

`WorkspacePages.stories.ts` renders the production login, deck, account, replay,
competition, privacy and error pages. `.storybook/page-imports.ts` supplies the
Nuxt auto-imports used by those SFCs, while `mocks/page-runtime.ts` provides an
isolated route, session and request handler. Every story resets its dependencies;
these requests never reach authentication or game services. The rules catalog
fixture comes from the Rust WASM `fewfc_rules_catalog` export.

The page stories cover both themes, loading failures, empty replay collections,
login failure, deck import feedback, replay deletion confirmation and focus
restoration. `workspace-layout.spec.ts` checks the composed pages at desktop and
mobile widths. These tests establish UI behavior only; the real service and
router contracts remain in integration and E2E suites.
