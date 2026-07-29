---
status: accepted
---

# Treat player-facing action detail as a contextual supplement

## Context

[ADR-0023](./0023-separate-action-detail-from-catalog-rule-text.md) moved
player-visible rule facts out of Formation Catalog prose and into a closed typed
Rules Engine contract. Requiring every offer to contain a non-empty,
independently complete detail then encouraged the implementation to repeat
Action identity and visible context: Formation and Profession Change Cards were
copied into `useCards`, a Pouch `sourceCard` was copied into `consumePouch`,
declared inputs and substitutions appeared both in the Action and its detail,
and section-wide behavior such as not ending the Action was restated per offer.
Rendering every typed fact as a sentence also repeated choices, shuffles, and
Echo behavior while exposing terms such as trusted randomness and engine
lifecycle boundaries.

The trade-off is between a detail that reads independently of its interface and
a concise explanation that relies on the decision context in which it is always
shown. Choose the contextual explanation: the Action control and accessible
decision context already own identity, while Action Detail exists only to
supply missing decision information.

## Decision

Player-Facing Action Detail is an optional, Player-scoped, state-specific
supplement to the complete decision context. An offer is complete when its
accessible Action label, visible state, and optional detail together let the
Player understand the choice. A detail may therefore be absent; a non-empty
`consequences` collection is not a completeness invariant.

Apply this inclusion test:

> If removing a clause still lets the Player correctly understand this offer's
> additional costs, results, conditions, follow-up timing, and specific
> exceptions from the Action label, accessible visible state, and official game
> terms, the clause is redundant.

The Action owns every input already chosen to identify the offer, including
Cards, targets, declared elements or levels, Formation roles, substitutions,
and a Secret Strategy's source Pouch. Those inputs must be distinguishable in
the Action control and its accessible state rather than recovered from a
tooltip. A consequence may still state what happens to such an input, but it
refers to the selected Card, source Pouch, or other game role instead of
repeating its identity.

Keep concrete supplemental costs, effects not otherwise presented, necessary
conditions and follow-up timing, current limits, and material rule exceptions.
Exclude identity, facts already on the decision surface, universal rules,
unspecific caveats, and negative engine-lifecycle or diagnostic statements.
Engine models may retain distinctions such as a printed element or trusted
random operation, but Player wording uses the shortest official game term
unless the distinction changes the choice. Thus ordinary wording says
`屬性`, not `印刷行屬`, and says `洗牌`, not that a trusted random procedure
performs it.

The Rules Engine decides which supplemental rule facts hold. The Web composes
those facts within its concrete decision surface without inventing or removing
a fact that changes the choice. Typed consequences are normalized semantic data,
not a sentence outline: related facts may be merged, and each Player-visible
fact is stated once. Wording expresses certainty naturally through terms such
as `可`, `若`, `隨機`, and an explicit timing rather than through translated
enum prefixes. Presentation starts with the main effect, keeps a cost or
condition beside the result it governs, follows in-game timing, and places a
current limit or material exception beside the affected fact.

## Consequences

Follow-up implementation should remove Action-identity data from the detail
contract rather than merely hide it in the renderer. This includes `useCards`,
duplicate declared inputs and substitutions, and `sourceCard` inside
`consumePouch` when the containing Secret Strategy option already owns the
source. It should also remove generic or engine-only detail variants that add no
decision information. Player-visible operations remain, using references such
as the selected Card or source Pouch.

Rust tests cover which supplemental facts an offer contributes and keep
player-scoping and serialization guarantees. Web tests cover representative
complete decision contexts and natural composition while retaining exhaustive
typed handling; they do not require one sentence per variant. Negative wording
assertions are optional regression guards, not a general testing requirement.
