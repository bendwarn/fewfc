---
status: accepted
---

# Expose Main Phase legality only as playable actions

The Rules Engine exposes every legal Main Phase command as a Player-scoped
Playable Action for one exact Action Card Selection, including Pass, Discard
Retrieval, Pouch Secret Strategies, and options from enabled Rule Modules. Web
adapters serialize and group those offers but do not maintain parallel
`canPass`, `canRetrieveDiscard`, optional-effect, or Secret Strategy capability
flags; non-Main lifecycles such as Initial Pouch Selection remain separate.
Options used to complete a Pending Choice, such as Chain's Secret Strategy
options, travel inside that typed Pending Choice payload rather than the Main
Phase interaction payload.
Initial, refreshed, and reconnected Player views carry the offers for an empty
Action Card Selection, while non-empty selections remain uncommitted
browser-local input and replace those offers through the same query. Every
serialized offer carries a `commandRole` of `activeEffect` or `action`, derived
from the Rules Engine's canonical Active-Effect Command and Action Command
classification rather than reclassified by Web code.

Trusted-randomness command variants continue an already selected Formation or
Spirit Skill through the existing Pending Randomness lifecycle. They are not
additional Playable Actions because they do not represent another Main Phase
choice.
