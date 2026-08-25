---
status: accepted
---

# Treat 離山 as Temporary Ability Loss

Although official 5.16 describes 離山 as making Profession Abilities and Spirit
Skills ineffective, this product treats it as **Temporary Ability Loss**: the
affected Player retains their Profession and Spirit identities but temporarily
lacks the complete official 「該職業之能力」set and all Spirit Skills. Options that
require those abilities are absent from Playable Actions and rejected without
costs or events when submitted directly, keeping the Rules Engine legality
contract from [ADR-0031](0031-expose-main-phase-legality-only-as-playable-actions.md)
authoritative instead of allowing a Player to spend a
cost or usage on an ability they temporarily do not have.

The loss is prospective. Consequences established by an already accepted
ability use remain, and canonical 離山 Status Effects, event shapes, Pending
Choices, and pure replay do not change. Permissions and prohibitions in the
Profession Ability Set are suspended alike; independent ordinary Profession
Changes remain legal, while Profession Formations and profession-granted paths
do not.
