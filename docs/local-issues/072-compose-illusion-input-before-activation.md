# 72 Compose Illusion input before activation

## Triage

ready-for-agent

## Problem Statement

Selecting two Cards currently makes the Rules Engine emit 25 complete Illusion
offers and 25 complete Phantasm offers: one for every element-and-level
declaration. The Web renders those otherwise identical offers as repeated
Ability buttons. This exposes candidate enumeration as interface structure,
makes the decision difficult to scan, and treats input selection as though the
Activated Profession Ability had already been committed.

Dark Spirit has a smaller version of the same presentation problem. It emits up
to two level-specific offers, and the Web renders them as same-named buttons
instead of using the level-picker interaction already established by Splendor.

## Solution

Represent Illusion and Phantasm as one offered Ability apiece with a typed
Action Input Requirement describing the legal Virtual Formation Card elements
and levels. The Web opens a client-local modal matrix from that offer. Selecting
one matrix cell completes and submits the existing ability command with
`declaredElement` and `declaredLevel`; opening or cancelling the matrix does not
activate the Ability, create canonical state, or create a Pending Choice.

Render Dark Spirit through the existing Splendor-style level picker while
retaining its current concrete engine candidates. Even a single legal Dark
Spirit level remains an explicit menu selection.

## User Stories

1. As a Player, I want one Illusion or Phantasm button after selecting its two
   cost Cards, so that I do not have to scan 25 repeated labels.
2. As a Player, I want to choose the Virtual Formation Card from a five-element
   by five-level matrix, so that all declarations are visible in one compact
   control.
3. As a Player, I want selecting a matrix cell to activate immediately, so that
   a redundant confirmation step does not slow the interaction.
4. As a Player, I want to cancel with a visible control or Escape while keeping
   my selected cost Cards, so that opening the matrix does not commit me.
5. As a reconnecting Player, I want an unsubmitted matrix draft to disappear,
   so that a browser-local selection is never mistaken for an activated
   Ability.
6. As a Player, I want Dark Spirit to use the same level-picker flow as
   Splendor, even when only one level is legal, so that the same button does not
   change behavior based on option count.
7. As a rules maintainer, I want the Rules Engine to describe legal Illusion
   inputs without multiplying Action offers, so that Web presentation does not
   depend on candidate enumeration.

## Implementation Decisions

- Add a closed typed Action Input Requirement to Profession Ability offers.
- The initial requirement variant describes a Virtual Formation Card using
  explicit legal elements and levels supplied by the Rules Engine.
- Emit one Illusion offer and one Phantasm offer for a legal two-Card selection.
  Their `declared_element` and `declared_level` are absent until command
  submission.
- Keep `declaredElement` and `declaredLevel` on the submitted command and
  validate both authoritatively in the Rules Engine.
- Do not create a canonical Pending Choice, Choice ID, continuation, Game Event,
  or persisted draft before command submission.
- Keep the existing read-only `playableActions` query. Opening and cancelling
  the matrix cause no additional engine command.
- Render the requirement as a modal 5×5 matrix. Do not repeat the cost Cards or
  the follow-up Formation scope inside the modal.
- Selecting a cell submits immediately and closes the modal regardless of the
  response outcome. While the request is in flight, prevent duplicate
  submission.
- A visible cancel control and Escape close the modal without submission and
  retain the selected Cards.
- Close an open modal if its Player, turn, phase, viewer, or offered Ability
  context becomes invalid.
- Keep Splendor unchanged. Group current Dark Spirit candidates into one
  Splendor-style level picker and always require an explicit menu-item click.
- Do not migrate Splendor or Dark Spirit to Action Input Requirement in this
  issue.

## Testing Decisions

- Add Rules Engine tests proving one Illusion and one Phantasm offer are emitted
  with the complete element-and-level requirement.
- Add Web serialization contract coverage for the exact camelCase
  `inputRequirement`, element, and level fields.
- Add Web interaction unit tests for requirement recognition, matrix lookup,
  command completion, and draft invalidation.
- Add component or end-to-end coverage proving one Ability button opens the
  matrix, a cell submits the declared element and level, cancellation submits
  nothing, and reconnect starts with no open draft.
- Add Web coverage proving Dark Spirit renders one trigger and uses an explicit
  level selection even with one candidate.
- Preserve direct command validation and replay coverage because the canonical
  activation shape and events remain unchanged.

## Out of Scope

- Changing the rules, cost, Formation Requirement, Virtual Formation Card, or
  replay semantics of Illusion or Phantasm.
- Persisting or synchronizing unsubmitted Action input.
- Representing Action Input Requirement as Pending Choice.
- Moving Splendor or Dark Spirit to the new engine requirement contract.
- Adding cost-Card summaries, Formation-scope prose, or a confirmation step to
  the Illusion modal.
- Introducing compatibility fields for the unreleased Web contract.

## Further Notes

- The glossary definitions for Activated Profession Ability, Action Input
  Requirement, Pending Choice, Virtual Formation Card, and Formation
  Requirement are authoritative.
- ADR-0025 continues to govern the canonical Illusion and Phantasm procedure.
- Existing dirty worktree changes, especially the in-progress Player-Facing
  Action Detail work, belong to the user and must be preserved.

## Blocked by

- none
