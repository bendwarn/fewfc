# 28 Split command errors by validation, implementation, and invariant layers

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 35.

## What to build

Replace the flat command error surface with explicit validation, rule-implementation, and engine-invariant layers. Player-facing illegal command errors should be distinguishable from incomplete rule implementations and impossible engine states, while preserving atomic command handling semantics.

## Acceptance criteria

- [ ] Command/application errors distinguish validation failures, rule implementation failures, and engine invariant failures.
- [ ] Existing validation errors are grouped under a validation error layer.
- [ ] `EffectNotImplemented` remains a rule implementation error.
- [ ] Engine invariant errors cover cases such as impossible draw availability, duplicate pending choices, duplicate covered passives, or zone ownership inconsistencies.
- [ ] Command and automatic advancement failures still emit no events and mutate no state.
- [ ] Tests cover one validation error, one rule implementation error, one engine invariant error, and atomicity for each category.

## Blocked by

- None - can start immediately

