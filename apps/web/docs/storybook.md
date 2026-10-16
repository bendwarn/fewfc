# Storybook

All 23 Vue components in `app/components/` have one or more stories in
`stories/components/`; deterministic shared view data lives in
`stories/fixtures/`. The preview loads the app's Tailwind stylesheet and starts
in the light theme. Storybook runs a Chromium smoke test for every story, and
stories with `play` functions also exercise browser interactions.

```sh
cd apps/web
bun run storybook       # http://localhost:6006
bun run test:storybook  # Starts Storybook and runs every story in Chromium
bun run build:storybook
```

`@storybook/addon-mcp` exposes the Storybook MCP endpoint at
`http://localhost:6006/mcp` while the development server is running. The
component manifest is enabled for component and story discovery.

Stories for API-backed competition controls begin in safe, unauthenticated
states. Tests that exercise Nuxt page/router wiring, composables, API transport,
or parent-child integration remain in Vitest or Playwright because those
contracts need their existing application context.
