---
status: accepted
---

# Compose Profession catalogs across Rule Modules

Keep one Player-owned Profession slot while composing the official Profession
catalog from every enabled Rule Module. Each module owns its Profession
definitions, Formations, and typed ability hooks; Profession IDs are globally
unique, and explicit parent links may cross module boundaries so rules such as
道法師 and 影戰士 can inherit Hero Schools abilities. Effective abilities are
derived from the composed catalog rather than copied into Game State.

Profession inheritance is an ordered acyclic graph rather than the current
single-parent tree: a definition may name multiple parents, and effective
abilities are deduplicated by stable ability identity. Transition predicates
remain separate from inheritance because a Profession's allowed current roles
and its inherited ability providers are not always the same relationship.

A single expanded Hero Schools catalog was rejected because it would make
Jianghu, Confluence Generation, and Dark Glimmer execute while their modules
are disabled and would concentrate unrelated rule behavior in `hero.rs`.
Separate Profession slots per module were rejected because the published rules
treat all Professions as mutually replacing roles and define transitions across
module boundaries.
