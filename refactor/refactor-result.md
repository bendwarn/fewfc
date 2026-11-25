# ADR-0032 Rules Test Refactor Checkpoint

This checkpoint describes a source inspection on 2026-11-20 at commit
`8758f3f` (plus the existing working tree). It is not a fresh claim-by-claim
audit or a declaration that ADR-0032's test refactor is complete. The working
records are [the inventory](rule-test-inventory.md) and
[the follow-up plan](rule-test-follow-ups.md).

## Current codebase

Rust integration suites are modules under `tests/single/`, registered by
`tests/single.rs`; the Cargo target is `single`. Historical references to
`tests/<suite>.rs`, line numbers and `cargo test --test <suite>` commands in the
inventory describe the older layout. The current suite also has matrices
embedded in `tests/single/rules_engine.rs` and `optional_rules.rs`.

Dedicated matrix files currently cover Pouch preparation and strategies,
Warrior, Echo, Jianghu, Spirit, Star, and Totem. The Totem files cover formation
and search, combat and Environment effects, and covered-passive timing; they
were not included in the original inventory. Existing Base matrices include
four-Player targeting, elemental history and Environment, Metamorphosis,
passives and Formation commitment. A filename or test name establishes the
location of evidence, not its completeness or compliance with the semantic
checklist.

The canonical passive outcome still uses `PassiveFlipOutcome::NoEffect {
grounds }`; current Pouch matrices include inapplicable and overlapping-ground
cases. The previous checkpoint's details must not be treated as a current
behavior specification: for example, the present Lure/Meditation matrix checks
that the option is removed and a stale Command is rejected.

## Inventory state

The preserved snapshot has 654 historical claim rows: 295 `retain`, 20 `merge`,
214 `rewrite`, and 125 `delete` dispositions. These are historical decisions,
not current completion counts. A source-name/title lookup found current
locations for 513 rows; 141 need manual reconciliation. These counts are rows,
not unique tests. A name match does not prove the assertion is unchanged, and an
unmatched name does not prove evidence is missing. The inventory predates some
current suites and must be extended and reviewed before further test removal.

The earlier checkpoint reported 35 removed legacy tests and 56 claims, but the
preserved inventory has since changed; do not use those old numbers to infer
current deletion or coverage totals. The earlier recorded full and targeted
runs are historical and do not validate the present checkout. For this refresh,
`cargo test --test single -- --list` succeeded and listed 390 tests; listing
tests does not execute or assess them.

## Next step

Reconcile each historical claim and the new tests with the current rules and
source, then record explicit current decisions and passing replacement cases.
Follow the batches and completion gate in [rule-test-follow-ups.md](rule-test-follow-ups.md).
