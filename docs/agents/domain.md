# Domain Docs

How engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This is a single-context repo for the CFECards rules engine.

Read these before domain-sensitive work:

- `CONTEXT.md` for the canonical glossary, relationships, and flagged ambiguities.
- `docs/rules-engine-decisions.md` for rules-engine decisions and follow-up context.
- `docs/rule.md` when the work depends on game rules.
- Relevant files in `docs/local-issues/` when working from an existing ticket.

If a referenced file does not exist, proceed silently.

## Use the glossary's vocabulary

When output names a domain concept in an issue title, refactor proposal, hypothesis, or test name, use the term as defined in `CONTEXT.md`. Avoid synonyms that the glossary explicitly rejects.

## Flag decision conflicts

If output contradicts an existing decision in `docs/rules-engine-decisions.md`, surface it explicitly rather than silently overriding it.
