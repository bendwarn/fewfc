# 31 Add serialization and filesystem persistence adapters

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` sections 4 and 33, and closed GitHub issue #22.

## What to build

Add real serialization-ready adapters around the persistence shapes. Event logs should be encodable/decodable as stable external data, and a filesystem adapter should save/load canonical event logs and optional snapshots without changing replay semantics.

## Acceptance criteria

- [ ] Persistence shapes derive or implement stable serialization/deserialization.
- [ ] Filesystem adapter implements event-log storage and snapshot storage ports.
- [ ] Serialized event logs include metadata, setup, recorded events, and optional snapshot data.
- [ ] Loading from serialized event logs reconstructs state through replay, not command re-decision.
- [ ] Snapshot loading is optional and never replaces authoritative event-log replay.
- [ ] Tests cover serialization round trip, filesystem save/load, missing snapshot behavior, and replay equivalence after load.

## Blocked by

- #30

