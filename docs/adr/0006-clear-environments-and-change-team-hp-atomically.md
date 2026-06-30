---
status: accepted
---

# Clear Environments and change Team HP atomically

A successful Void Meridian-Severing Technique resolves Environment Clearing
and both Teams' 20-point HP losses in one `EnvironmentCleared` canonical event.
Projectors apply every included HP delta before evaluating the Game Outcome, so
simultaneous defeat produces a Draw and cannot depend on serialized event order.
Separate `HpChanged` events were rejected because they expose a false
intermediate winner between changes that the rules treat as simultaneous.
