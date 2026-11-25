# ADR-0032 Rules Test Follow-ups

This is the current working plan for the claim-level inventory in
[rule-test-inventory.md](rule-test-inventory.md). The original audit predates
changes to the Rust integration-test layout and behavior. Its classifications
and named replacements are starting points for review, not proof that a present
claim is covered. The [checkpoint](refactor-result.md) records what was
inspected when this plan was refreshed.

## Reconcile the inventory first

1. For every historical row, inspect the current test and its assertions. A
   same-name locator is navigation only; confirm behavior, scope and lifecycle.
   Resolve rows with no locator by finding a renamed test, an intentional
   deletion and its replacement, or a genuinely missing claim. Record the
   evidence and current decision in the inventory before deleting any test.
2. Audit rules behavior introduced or changed since the original snapshot,
   including the three Totem matrix suites and subsequent changes to Pouch,
   Pending Resolution and game conclusions. Add claim-level rows where needed;
   do not assume the 654 historical rows cover the current codebase.
3. Recheck the original merge, rewrite and delete decisions against current
   named tests. In particular, the old Lure/Meditation entry describes an
   accepted activation with suppressed effects, while the current
   `pouch_lure_meditation_matrix_removes_the_option_and_rejects_a_stale_command`
   tests removal of the option and rejection of a stale Command. Determine
   which current rule and evidence apply before carrying the old claim forward.
4. Have the audit owner and primary implementer settle ambiguous classifications
   and replacements in batches. Keep independent invariants, matcher, catalog,
   serialization, validation atomicity, public-view and replay contracts.

## Review interaction coverage by boundary

Use `docs/rule.md` and ADR-0032 to assess actual assertions, not test names.
The historical follow-ups identify the following areas for reconciliation;
existing tests may already cover some cases, so these are review targets rather
than a claim that each case is missing:

- **Pouch and Personal Deck:** preparation and privacy, independent selection,
  strategy choice and shuffle lifecycles, Chain, Sheep Stealing, Steal the Beam,
  Lure, Return Soul and Deceive Heaven. Recheck changes to typed strategy
  decisions and Pending Resolution.
- **Base:** Start/deal and Turn lifecycle, Formation commitment, targeting in
  two and four Players, Environment and Shield, elemental history, Sacred
  Beast, Void Meridian, Metamorphosis, Chaos, Radiance and Discard Retrieval.
- **Covered passives, Hero Schools, Spirit and Star:** legal profession
  progression, scope and no-effect grounds, Warrior proficiency, Spirit Skills,
  Star ownership/substitution/breaking and terminal ordering.
- **Echo and Tribulation:** delayed scheduling, automatic resolution, typed
  choices and randomness, non-recursion, no-effect consumption, replay and
  relevant Player privacy.
- **Jianghu, Confluence Generation and Dark Glimmer:** profession and effect
  scope, Formation and Ability sources, ongoing effects, terminal ordering,
  card movement and replay.
- **Totem:** inventory the existing acquisition, combat, Environment and
  covered-passive timing tests, then assess their distinct interactions with
  other modules and unaffected consequences.
- **Web seams:** confirm the representative privacy, reconnect, typed choice,
  configuration and endgame cases still exist and are paired with Rust rules
  evidence; browser tests are not the sole Rules Engine evidence.

For each needed matrix, establish effects under test with legal Commands,
provide independently failing baseline, modifier and combined cases, and check
canonical outcomes, affected and unaffected consequences, costs, Card movement,
final state and replay as applicable. Compare multiple No-Effect Grounds as a
set, reject duplicates, and do not infer domain priority from serialization.

## Validation and completion

The Rust integration suites now share the `single` Cargo target. Run a focused
suite with, for example, `cargo test --test single pouch_interaction_matrix`,
then run `cargo test` and `git diff --check` after the implementation and
claim-level review are complete. Use additional Rust Web or Playwright checks
when those seams change. Historic `cargo test --test <old-suite>` commands in
the inventory are evidence of earlier runs, not current invocation instructions.

Completion requires an up-to-date inventory, named and passing replacements for
every claim whose old test is removed, resolution of classification conflicts,
and review against the semantic checklist in `docs/rule.md`. No completion
claim follows from this documentation refresh alone.
