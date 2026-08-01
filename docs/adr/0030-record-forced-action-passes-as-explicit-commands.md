---
status: accepted
---

# Record forced action passes as explicit commands

When an Action Pass is the sole Playable Action, the Rules Engine advances
without awaiting Player input but records the same explicit `PassAction`
Command used when a Player declines optional Active-Effect Commands. This keeps
Command validation, Game Events, and replay consistent at the deliberate cost
of a Command source that was selected automatically rather than submitted by a
Player. Forced advancement recognizes only an empty Action Card Selection whose
authoritative Playable Action result is exactly one Action Pass; it does not
rederive pass legality through a parallel predicate. An Action Pass may
coexist with optional Active-Effect Commands but never with another Action
Command, so only the exact one-offer result advances automatically. The
Playable Action carries the authoritative `PassActionReason`; automatic
application copies that reason into the Command, whose normal validation still
protects against stale state.
