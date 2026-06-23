# CFECards Web UI

Nuxt adapter for the CFECards Rust rules engine.

The UI consumes viewer-filtered Public Game State and Public Event Feed data. It should not use canonical Game State or canonical Game Events directly as browser replay sources.

## Setup

Make sure to install dependencies:

```bash
bun install
```

## Development Server

Start the development server on `http://localhost:3000`:

```bash
bun run dev
```

## Production

Build the application for production:

```bash
bun run build
```

Locally preview production build:

```bash
bun run preview
```

See `docs/deployment.md` for the Cloudflare deployment path.
