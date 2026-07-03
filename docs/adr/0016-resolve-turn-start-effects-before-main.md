---
status: accepted
---

# Resolve Turn Start effects before Main

Turn Start is a resolving stage rather than an immediate transition into Main:
the engine records entry into the turn, expires effects due at that timing,
resolves delayed Melody effects such as Echo and 變宮‧植土, and pauses for any
required player or randomness decisions. Main becomes available only after the
stage completes, so expiring effects are absent before delayed effects execute
and no Player can act while a Turn Start continuation remains unresolved.
