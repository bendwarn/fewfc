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

An internal deep `pending_resolution` module owns that resume seam. Its single
interface accepts an opaque, already validated Choice or Randomness input,
captures the semantic Pending Resolution before projection clears the waiting
state, emits and projects the canonical `ChoiceMade` or `RandomnessResolved`
event, and exhaustively delegates the typed rule continuation. Raw external
input validation remains with `pending_choice` and `randomness`; Rule Module
handlers remain typed functions that return canonical events, while
`pending_resolution` owns their projection and ordering. One semantic Pending
Resolution may pass through multiple sequential Choice and Randomness waits.

The module also owns consequences that belong to this resume sequence,
including Discard Shuffle ordering, while delegating Tailwind recovery to the
Confluence Rule Module and Formation Use completion to `formation_use`. It may
finish by creating the next waiting input, completing the Formation Use, or
concluding the Game. It then requires at most one waiting input and requires
that input to match the remaining Pending Resolution. An inconsistent internal
waiting state is `EngineInvariantError::InvalidPendingResolution`; invalid
external answers remain validation errors.

Pending Resolution completion deliberately does not cross the Player Decision
boundary. Application orchestration continues to own automatic advancement to
the next Player Decision, forced Pass handling, and `RecordedDecision`
grouping. This keeps the resume module independent of application batching and
replay verification.

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
is retained. The refactor does not otherwise change the canonical event
sequence or schema, `RecordedDecision` sources or batches, Public View, or
replay verification. Existing dispatch-detail tests may be removed only after
their claims have named replacement evidence as required by ADR-0032;
interaction matrices remain intact. The closed typed dispatch does not add a
runtime trait, registry, or adapter.
