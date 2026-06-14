# fewfc

Deterministic Rust rules engine for CFECards.

The first implementation target is a pure Rust library crate using Clean Architecture boundaries. The canonical game record is an event log; snapshots are optional loading checkpoints.

## Project Layout

```text
src/
  domain/          # core game concepts and invariants
  application/     # command handling, turn orchestration, replay services
  rules/           # formation and effect registries
  ports/           # storage and integration traits
  infrastructure/  # serialization/storage adapters and deterministic helpers
```

Design decisions are tracked in `docs/rules-engine-decisions.md`.

## Development

```bash
cargo test
```
