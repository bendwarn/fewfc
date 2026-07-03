---
status: accepted
---

# Record delayed-effect lifecycle events

Echo and 變宮‧植土 record explicit canonical events for decline, scheduling,
resolution start, completion, and expiry instead of requiring replay, reconnect,
or event presentation to infer semantic boundaries from lower-level state
deltas. Their main effects continue to emit the existing HP, Status, Card move,
choice, and shuffle events, while Echo and 植土 retain distinct lifecycle event
identities because neither is a new Formation Use.
