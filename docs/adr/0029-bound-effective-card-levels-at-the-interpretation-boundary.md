---
status: accepted
---

# Bound effective Card Levels at the interpretation boundary

The Base Ruleset fixes every consumable Card Level to the inclusive range one
through five. We distinguish Printed Card Level from Effective Card Level:
rules read the bounded Effective Card Level unless they explicitly require the
printed value. Physical Cards obtain their effective element and level through
one Effective Card Facts Resolver, while Virtual Formation Cards use the same
bounded level type without participating in Card Interpretation Layers. Rule
Modules and Game Setup cannot override these bounds.

Semantic canonical events remain specific to their rules and project their
effects into one ordered collection of Card Interpretation Layers. The resolver
composes those layers in canonical effect order and applies the level bound
once, after the complete composition. A relative adjustment may cross a bound
while composition is in progress; an absolute level assignment must already be
within one through five, and an invalid assignment makes command handling or
replay fail instead of being silently corrected. Printed and Effective Card
Levels are distinct domain types so ordinary rule consumers cannot bypass the
resolver accidentally.

This deepens ADR 0011's dimension-composition decision and retains ADR 0025's
separation between physical interpretations and Virtual Formation Cards. We
reject module-local clamps, hard-coded precedence over scattered module state,
and per-layer clamping: they respectively permit omissions, obscure canonical
effect order, and change the agreed final-composition semantics.

Property tests must prove that arbitrary valid layer sequences always resolve
to levels one through five and that bounds apply only after full composition.
Conformance tests must cover every semantic event-to-layer mapping and enforce
that rule consumers accept Effective rather than Printed Card Levels. A
captured-room regression test is unnecessary. Web DTO and hand-UI presentation
of effective levels remain outside this decision.
