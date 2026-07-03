---
status: accepted
---

# Use typed Pending Choice answers

Pending Choice commands use a closed typed answer contract for Cards, Player,
Formation, and Decline answers, and each Pending Choice kind validates the
answer variants it accepts. This replaces the card-only answer payload instead
of adding one command per Formation or encoding non-card choices as strings or
empty card arrays, so canonical commands, replay, Web DTOs, and future
effect-generated choices share one explicit protocol.
