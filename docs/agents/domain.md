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

When writing or updating rule references, ADRs, or local issues, write prose in
Traditional Chinese and use the verified original Chinese names of official game
rules, Formations, and abilities. Check the applicable rulebook version and its
supplements; use `CONTEXT.md` for established mappings and record missing ones
there. Retain an established English domain term alongside its Chinese name when
needed to identify a project-specific concept. Distinguish product terminology
from official wording. If the original cannot be verified, retain the existing
name and identify the source gap rather than inventing a translation. Preserve
code identifiers, commands, paths, links, status labels, and historical evidence.

## Flag decision conflicts

If output contradicts an existing decision in `docs/rules-engine-decisions.md`, surface it explicitly rather than silently overriding it.
