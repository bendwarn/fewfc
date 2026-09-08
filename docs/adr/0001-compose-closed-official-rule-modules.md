---
status: accepted
---

# Compose the Base Ruleset with closed official rule modules

The Rules Engine always runs the Base Ruleset and may enable compile-time
Rule Modules from a closed official catalog. A Ruleset configuration is the Base
Ruleset plus its enabled modules; Game Setup and Game State retain that
configuration so command handling, automatic advancement, Formation queries,
replay, and Public View use the same rule knowledge without caller selection.

The deep rule-execution interface owns setup rules, event decisions, automatic
advancement, and playable-Formation queries. Formation registries, matchers,
Effect Plans, resolvers, module hooks, and Pending Choice continuations remain
implementation details. Tests use this interface as their primary test surface
and run conformance cases with each supported module configuration.

## Considered options

- A separate Star Ruleset was rejected because the Base Ruleset is mandatory and
  Star rules only augment it.
- A public trait for downstream rule modules was rejected because the closed
  official catalog does not justify freezing an external interface.
- Runtime plugins or data-loaded rule modules were rejected because they add
  versioning and safety costs to deterministic replay and Wasm deployment.
- Separate query and command modules were rejected because Formation matching
  and command validation must use the same rule knowledge.
- Separate canonical schemas were rejected because the Base Ruleset and enabled
  modules use the same Rules Engine, projection, replay, and viewer-filtering
  semantics.

## Consequences

Rule Version identity and theme selection below are superseded by
[ADR-0038](0038-bind-games-to-rule-versions.md). Record schema compatibility
remains a separate decision.

Adding an official Rule Module requires extending the exhaustive catalog and may
extend the shared canonical schema. Available modules are enabled by default in
new Web rooms and remain independently configurable; disabling all modules
leaves the complete Base Ruleset active. Before the first public release,
persisted Game Record compatibility across schema changes is not guaranteed; no
version or migration mechanism is added yet.
