---
status: accepted
---

# Model and test rule interactions explicitly

Rules behavior uses interaction matrices as its primary test evidence because an
isolated effect test can prove that one rule works alone while missing failures
at scope, timing, commitment, prevention, and no-effect boundaries. Each matrix
contains independently failing baseline, modifier, and combined-interaction
cases, establishes the effects under test through legal Commands, and checks
the canonical outcome, affected and expressly unaffected consequences, costs,
Card movement, and replay. Narrow tests for invariants, matchers, catalogs,
serialization, and atomic failure remain because they prove different contracts
more precisely.

Interaction coverage is selected by distinct typed hook, scope, source, or
lifecycle rather than by a complete Cartesian product of rules. Effects that
make another effect ineffective, immune, prevented, countered, suppressed,
ignored, copied, substituted, delayed, or only partly effective require a
reasonable within-module and cross-module interaction when those paths exist.
Existing Rust rules tests must be inventoried claim by claim before any is
removed; a test may be deleted only after every unique claim has named
replacement evidence. Playwright retains representative Web seams and does not
become the sole evidence for Rules Engine behavior.

An applicable effect has one resolution outcome even when several independent
rules make it have no effect. The canonical no-effect outcome records every
independently sufficient **No-Effect Ground** without designating a primary
ground or assigning rule priority to collection order. Applicability is checked
before grounds are collected, so a rule that could not affect the incoming
action does not acquire unrelated grounds. The previous single `reason` shape
is replaced directly by `grounds`; no historical reconstruction or compatibility
primary field is required because affected canonical records have not been
created.

## Consequences

A single audit owner maintains the ignored `tmp/` inventory and reviews it with
the primary implementer in batches, surfacing ambiguous claims and classification
conflicts before continuing. The complete inventory precedes test deletion,
although confirmed product defects may be fixed earlier. Repository review
applies the semantic evidence checklist in `docs/rule.md`; CI does not infer
interaction quality from test names or case counts.
