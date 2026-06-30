---
status: accepted
---

# Record every Environment Transfer

Every Sacred Beast Formation Use emits an `EnvironmentTransferred` canonical
event after its attack resolves, including when the previous and resulting
Environment have the same element. Although an equal-element transfer does not
change projected state, retaining the semantic fact keeps event feeds and
future cross-module reactions from having to reinterpret Formation history
under later rule versions.
