---
status: accepted
---

# Model player-facing action detail as typed rule consequences

## Context

Formation Catalog text and the explanation shown before a Player commits an
Action serve different purposes. Catalog text describes a Formation in
isolation. A Player-Facing Action Detail must explain the consequences of the
specific Action currently being offered, including state-dependent costs,
substitutions, follow-up choices, trusted randomness, delayed effects, and Rule
Module exceptions.

The previous Web contract exposed a `summary` string copied from catalog
`rule_text`. The Web then appended rule knowledge through Formation-ID policies
and separate Secret Strategy prose tables. Echo made the resulting ownership
problem visible: Rust owned the legal action and Echo lifecycle, while
TypeScript separately described the Echo Cost and schedule. Either side could
change without forcing the other description to change.

## Decision

The Rules Engine represents every player-visible consequence as a closed typed
`RuleConsequence`. Consequence variants cover at least costs, immediate effects,
follow-up choices, trusted randomness, delayed effects, rule exceptions, and
substitutions. They are rule facts rather than localized prose, canonical Game
Events, or simulations of the final Game State.

A required `PlayerFacingActionDetail` is embedded in each pre-commit Action
offer. It is:

- Player-scoped and limited to information that Player may already see;
- state-specific to the same snapshot that produced the legal Action;
- complete for the consequences the Player needs to understand before commit;
- ordered so its clauses form one coherent explanation; and
- presentation-neutral, with the Web owning localized wording and layout.

Rule behavior supplies reusable consequence descriptors next to its execution
logic. A central Player-Facing Action Detail module composes those descriptors
with the current public state and verifies completeness. The Web exhaustively
renders the serialized variants. It does not select rule prose by Formation ID.

The model applies to Formation Use, Profession Change, Activated Profession
Ability, Spirit Skill, direct Pouch Secret Strategy offers, and Discard
Retrieval. Pass may retain a fixed concise label without rich detail.

Pending Choice remains a separate lifecycle. Its descriptor may reuse the same
`RuleConsequence` values to explain a choice, but an Action Detail does not
materialize canonical choice options, carry a Choice ID, validate an answer, or
resume a continuation. A `SecretStrategyOption` may therefore support either a
direct Pouch trigger or a Chain Pending Choice without becoming a universal
standalone Action.

The existing `playableActions`, Secret Strategy option, and Discard Retrieval
workflows do not need to become one transport union. They share the detail
contract while retaining their distinct command inputs and orchestration.

An accepted command does not echo its detail back to the Rules Engine. The
detail describes the offered snapshot; canonical command validation remains
authoritative.

Action Details state known commitments rather than promise a fully resolved
outcome. They distinguish guaranteed, conditional, random, follow-up, and
scheduled consequences, and never reveal hidden opponent information or an
undecided random result.

Catalog prose may still be displayed as supplementary reference material, but
it is not a completeness fallback. Constructing an offer without its required
typed detail is a development invariant violation. New rule variants must add
their consequences and Web rendering explicitly.

## Consequences

After the Pending Choice lifecycle work is complete, the Web seam changes
atomically: playable-action `summary`, Formation presentation `policy`, and
rule-specific TypeScript prose maps are removed in favor of the required typed
detail. Pre-deployment code does not retain a dual-field compatibility layer.
Event-feed summaries and Formation Catalog summaries remain separate and are
not part of this migration.

Validation is split by responsibility:

- Rust rule tests assert the consequences produced by each Action family and
  representative Rule Module exceptions.
- Completeness tests ensure every generated Action offer has a complete detail.
- serialization contract tests lock tagged variants and exact camelCase Web DTO
  fields;
- TypeScript tests exhaustively render consequence fixtures; and
- end-to-end coverage remains focused on representative interactions such as
  Echo and Chain rather than duplicating every rule case.

This design increases the explicit work needed when a new rule mechanism is
introduced. In return, the compiler and contract tests expose missing player
explanations instead of allowing incomplete prose to ship silently.
