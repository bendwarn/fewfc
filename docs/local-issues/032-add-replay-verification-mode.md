# 32 Add replay verification mode

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 33.

## What to build

Add an optional verification mode that can rerun command decisions or automatic advancement against a known history and compare the produced canonical events with the stored event log. Pure replay must remain unchanged and should continue applying recorded events exactly.

## Acceptance criteria

- [ ] Pure replay continues to apply canonical events without recomputation.
- [ ] Verification mode can compare expected automatic events with recorded automatic events.
- [ ] Verification mode can compare expected command-produced events with recorded command-produced events when command context is available.
- [ ] Verification failures report sequence and event mismatch details.
- [ ] Tests cover successful verification, automatic event mismatch, command event mismatch, and pure replay independence.

## Blocked by

- #29

