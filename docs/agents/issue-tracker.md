# Issue Tracker

Issues for this repo live as markdown files in `docs/local-issues/`.

## Conventions

- The issue index is `docs/local-issues/README.md`.
- Issue files are named `NNN-short-title.md`, where `NNN` is the local issue number.
- Existing local issues are follow-up work derived from `docs/rules-engine-decisions.md`.
- Keep issue language aligned with the domain glossary in `CONTEXT.md`.
- Record completion by moving the issue link from "Open Local Issues" to "Completed Local Issues" in `docs/local-issues/README.md`.

## When a skill says "publish to the issue tracker"

Create a new markdown file in `docs/local-issues/` using the next available local issue number, then add it to the "Open Local Issues" section in `docs/local-issues/README.md`.

## When a skill says "fetch the relevant ticket"

Read the referenced file in `docs/local-issues/`. If the user gives only a number, match it to `docs/local-issues/<NNN>-*.md`.

## GitHub

The repository has a GitHub remote, but local markdown is the active issue tracker for this project unless the user explicitly asks to use GitHub Issues.
