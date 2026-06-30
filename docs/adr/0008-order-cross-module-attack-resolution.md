---
status: accepted
---

# Order cross-module Attack resolution

Attack resolution first applies legal Card interpretations and matches the
Formation, then computes Attack Points including Profession modifiers, evaluates
point-based qualifications, resolves Counter Effects, applies five-element
interaction, Environment Effects, and Shields, and finally resolves
post-Formation consequences. Canonical Attack events retain both Attack Points
and the final damage or recovery result.

This order keeps point-based rules such as Windwalking and Star Summoning
independent from damage transformations such as Environment Effects. Allowing
each Rule Module to wrap the entire resolver was rejected because the result
would depend on module registration order rather than explicit game semantics.
