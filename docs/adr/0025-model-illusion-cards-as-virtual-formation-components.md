---
status: accepted
---

# Model Illusion cards as virtual Formation components

Illusion and Phantasm create a Virtual Formation Card after their Player
discards two physical Cards; they do not reinterpret a third Card Instance.
The Virtual Formation Card has a fixed source ability, element, and level and
joins the subsequent Formation Composition without entering a hand, Deck, or
Discard Pile. A turn-scoped Formation Requirement commits Illusion to the
corresponding five-element strike, commits Phantasm to a Base Ruleset
Formation, and records the completed Formation Composition canonically so
replay and audit never need to infer a virtual component from earlier events.

Physical Card Instances and Virtual Formation Cards share effective element,
level, match-slot, point, and component-count calculations, but only physical
Cards participate in selection, visibility by zone, Card movement, and effects
that require a Card Instance. Virtual facts are public when created, cannot be
changed by Card Interpretation Layers or Star Element Substitution, and are not
represented as a fabricated Card Instance. The rejected third-physical-Card
model made Formation matching disagree with point and effect resolution and
could not faithfully represent the official virtual-card procedure.

Old records that used a third physical Card for Illusion or Phantasm are not
migrated. Records unrelated to the corrected preparation semantics retain
their existing canonical shapes.
