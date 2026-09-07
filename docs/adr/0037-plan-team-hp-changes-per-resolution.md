---
status: accepted
---

# Plan Team HP changes per resolution

HP facts are observable game records, but several rules can affect the same
Team inside one resolution. Deriving every fact afresh from canonical state
loses call order, obscures clamping/prevention, and lets post-effect rules
accidentally recurse.

Each outer rules resolution therefore validates and owns one ordered Team-HP
ledger. Formation effects borrow a scoped child session from that ledger. The
session reports only canonical-order Teams that actually lost HP; Shared Fate
is planned afterward with the outer ledger and can never re-enter the session.
Canonical events retain their semantic owner, while the HP delta itself is
constructed only by the HP module.

Pending choices and randomness do not retain a live ledger. Their continuation
starts a fresh outer resolution from canonical state and records a typed Melody
execution origin. Echo is not a Formation effect, so it cannot acquire
Formation-only prevention or Shared Fate by inference from ambient state.

## Considered options

- A global HP-source enum was rejected: semantic events already own their
  reason, eligibility, and follow-ups, and a universal source would duplicate
  those rule concepts.
- Aggregating attack HP changes by Team was rejected: it destroys sequential
  same-Team changes. Attack records instead carry an ordered vector with a
  small role classification only where a mixed collection needs one.
- Serializing an in-progress ledger through Pending Resolution was rejected:
  command replay must reconstruct continuation behavior from canonical state.
