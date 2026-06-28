# 36 Harden production environments

## Triage

ready-for-agent

## What to build

As an operator, I want development conveniences to be isolated from production-like
environments so staging and production always create and present games according to
the published rules.

Only local development may expose fixtures, fixture-based setup fallbacks,
diagnostic UI, or development-only adapters. Staging and production must have the
same gameplay, permissions, and player-facing interface; they differ only in
deployment origin, bindings, secrets, and persisted data.

## Acceptance criteria

- [x] The application has an explicit environment distinction for local development,
      staging, and production.
- [x] `APP_ENV=development|staging|production` is the sole environment policy input;
      application behavior is not inferred from URLs, `NODE_ENV`, or Wrangler
      environment names.
- [x] A production-like deployment with a missing or invalid `APP_ENV` fails clearly
      instead of defaulting to development behavior.
- [x] Staging and production use identical gameplay rules, permissions, and
      player-facing UI.
- [x] Local development is the only environment allowed to expose test conveniences.
- [x] Production-like game creation uses authoritative room participants plus
      ruleset-defined defaults rather than fixture players, fixture order, or fixed
      fallback seeds.
- [x] A room supplies the actual participants and player count; the selected ruleset
      supplies defaults such as starting health and the official deck.
- [x] First player, four-player teams and turn order, and the shuffle seed are
      randomized when the match starts.
- [x] Generated setup and random results are persisted once and reused for reconnect
      and replay.
- [x] The Base Ruleset exposes a production setup path that combines actual room
      participants with official ruleset defaults and card composition.
- [x] `sample_game_setup` and fixture identities or ordering are limited to tests and
      the development-only surface.
- [x] Production code does not obtain official card composition indirectly through a
      sample or fixture setup.
- [x] Player-facing production-like pages contain no diagnostic controls, fixture
      identities, internal event names, or prototype-only labels.
- [x] Anonymous guest play remains available in staging and production as a product
      feature, labeled `以訪客身份遊玩` rather than as a quick-start or test shortcut.
- [x] Guest display names are product-appropriate generated identities and never
      expose fixture names such as Alice or Bob.
- [x] Redundant prototype kickers (`GAME LOBBY`, `MATCH READY`, `WAITING ROOM`, and
      `GAME SET`) are removed rather than translated.
- [x] Initial client state contains no prefilled fixture identity such as
      `玩家 Alice`.
- [x] A new room defaults to the editable name `{host display name}的房間`
      instead of the prototype name `五行練習場`.
- [x] Development-only server routes and adapters are unavailable in staging and
      production.
- [x] Development-only UI is excluded from staging and production bundles rather
      than merely hidden.
- [x] Direct requests to development-only APIs return `404` in production-like
      environments and cannot invoke local processes.
- [x] Server-side authorization and rule validation remain authoritative regardless
      of which controls the client renders.
- [x] Normal login, room, and match screens retain the production interface even
      during local development.
- [x] Any fixture or manual engine controls live only under a development-only
      `/dev` route that is absent from staging and production.
- [x] Local play and `/api/local-game` are development-only; staging and production
      create matches exclusively through online rooms.
- [x] Missing or placeholder production-like bindings and secrets fail deployment or
      startup clearly instead of silently using local defaults.
- [x] Wrangler defines explicit, isolated `staging` and `production` environments,
      each with `APP_ENV`, origin, storage, and Durable Object bindings appropriate
      to that environment.
- [x] Package scripts expose only explicit production-like deployment commands such
      as `cf:deploy:staging` and `cf:deploy:production`; an unqualified `cf:deploy`
      cannot publish local defaults.
- [x] Environment-matrix tests verify that `/dev` and `/api/local-game` work only in
      development and return `404` in staging and production.
- [x] Production-like UI tests verify that fixture identities, prototype English
      kickers, and development controls are absent.
- [x] Configuration tests verify that missing or invalid `APP_ENV` and placeholder
      production-like bindings fail clearly.
- [x] Match-start tests verify that room participants, generated setup, randomized
      order, official deck, and shuffle seed are persisted.

## Non-goals

- Changing the game rules.
- Making staging share production data or secrets.
- Adding a beginner or learning mode.

## Blocked by

- None - can start immediately
