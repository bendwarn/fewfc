# Domain Docs

How engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This is a single-context repo for the CFECards rules engine.

For domain-sensitive work, locate the relevant entries or sections in these sources. Full-document reading is unnecessary unless the task spans the whole model:

- `CONTEXT.md` entries for concepts, relationships, and ambiguities touched by the task.
- `docs/rules-engine-decisions.md` sections and linked ADRs for decisions affecting the change.
- `docs/rule.md` when the work depends on game rules.
- Relevant files in `docs/local-issues/` when working from an existing ticket.

Skip missing optional background material. If a missing rule, decision, or ticket is necessary to establish the requested behavior, state the gap, continue independent work, and obtain the missing contract before making a dependent change or claiming correctness.

## Use the glossary's vocabulary

When output names a domain concept in an issue title, refactor proposal, hypothesis, or test name, use the term as defined in `CONTEXT.md`. Avoid synonyms that the glossary explicitly rejects.

## Flag decision conflicts

If output contradicts an existing decision in `docs/rules-engine-decisions.md`, surface it explicitly rather than silently overriding it.
