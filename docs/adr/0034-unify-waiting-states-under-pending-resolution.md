---
status: accepted
---

# Unify waiting states under Pending Resolution

Pending Resolution is the one serialized owner of an accepted rule flow while
it waits for player input or trusted randomness. Pending Choice contains only
the closed answer requirement and Pending Randomness contains only the random
operation, source order, and validated result; neither carries a continuation.
The Rules Engine resumes the Pending Resolution generically after either input,
so card supply and future rule flows do not add Rule Module-specific randomness
or choice-routing variants.

## Considered options

- Keeping separate Choice Continuation and Randomness Continuation unions would
  preserve the existing shapes but continues to classify rule flow by its
  waiting mechanism and distributes retry logic among Rule Modules.
- Generic application callbacks cannot be persisted, replayed, or validated.
- Retaining ADR-0026's separate continuation ownership only for Pending Choice
  would avoid a wider migration but leaves two competing owners for the same
  rule flow.

## Consequences

This supersedes ADR-0026 only for continuation ownership; its closed public
Pending Choice contract and Web interaction seam remain. Deployment cuts over
to record schema 7 and purges existing active and finished Game Records
rather than migrating their typed continuations. Public Views expose only the
waiting input appropriate to each viewer. Tests cross the Pending Resolution
interface for both choice and randomness pauses; no legacy-record replay path
is retained.
