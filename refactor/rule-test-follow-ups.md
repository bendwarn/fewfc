# ADR-0032 Rules Test Follow-ups

The [inventory](rule-test-inventory.md) has current source locations and decisions
for all 654 historical rows. The second pass reviewed the 513 remaining
name matches and added 149 current claim rows, including the three Totem
Formation suites, typed Pending Resolution, Pouch strategy decisions, Rule
Versions and Game Conclusion. Every test in the current Rust `single` target
has a named source location. The
[original checkpoint](refactor-result.md) preserves the starting counts.

## Replacements for the first pass's open claims

- **Pouch preparation.** The matrix checks 55 and 54 remaining Cards in the
  two Personal Decks after Pouch placement and the 4/5 opening deal.
- **Watch the Fire.** One case compares Metal Strike with and without Watch
  against a legal Barrier. The other compares Void Meridian with and without
  Watch and checks that both branches clear the Environment and discard the
  submitted Cards. Watch preserves both Teams' HP in the combined case.
- **Covered Defense.** The live state asserts the exact Cards left in hand,
  the covered Cards in the Formation Area and the Turn Draw transition.
- **Ringing Metal.** The same two printed Metal Cards offer both Ringing Metal
  and Weapon. A second case deals the entire fixture Deck and checks the
  typed empty Discard Shuffle, the absence of a Card choice and replay
  agreement at the pending boundary.
- **Discard Retrieval under Cannot Act.** The Personal Deck case checks the
  derived Card, each Card zone, the public reveal on Deck Top and an HP
  charge of twice its printed level. The existing shared-Deck case remains.
- **Barrier under Metal Environment.** The four-Player interaction checks a
  nonzero Shield from a legal Barrier. A later Barrier commits and discards
  its Cards in Metal Environment without changing that Shield.

## Contract decisions

- Lure removes the affected ability offer and rejects a stale Command as
  Temporary Ability Loss. [ADR-0033](../docs/adr/0033-treat-lure-as-temporary-ability-loss.md)
- The typed Pending Resolution lives on the canonical event envelope.
  The Web randomness request keeps its operation and order fields, and Chain
  answers use the closed Pouch decision shape. [ADR-0034](../docs/adr/0034-unify-waiting-states-under-pending-resolution.md)
- Instant Shadow Death leaves half the current HP rounded down. The Web
  Discard Retrieval seam records retrieval before the forced Cannot Act pass.
  Both have explicit assertions.

## Echo integration rewrites

Eight existing tests now use official game setup and recorded Commands.
Their current evidence is linked from the inventory.

- **Ringing Metal.** The shared Deck case checks choice privacy, public reveal,
  the exact remainder shuffle and Echo-cost continuation. Normal turns
  build the Discard and exhaust the Deck before the recycle case. It checks
  both randomness requests separately and verifies their final order. The
  Personal Deck case checks the performing Player’s search and shuffle, the
  other Player’s Deck and Hand, and the discard destination. The empty-piles
  case still covers the typed empty recycle request.
- **Split Earth.** Legal turns show Weapon damage prevented on the target’s
  next turn and restored after expiry. A separate comparison checks Falling
  Wood recovery and the Echo-cost offer with and without matching suppression.
  The paid Echo inspects a changed Hand and resolves before normal Turn Start.
  The four-Player case checks seat order and inspection privacy. White Tiger
  keeps its Environment transfer in all three suppression-origin branches,
  with damage prevented only by Echo-origin Split Earth.

Each rewritten test checks event replay and recorded-decision verification
against live state. The inventory retains the historical columns and records
the current assertions and decisions separately.

## Integration test reconciliation

The review of 62 integration-test candidates is complete. It produced 56
legal-command rewrites, confirmed existing legal matrix evidence for three
cases, and retained three independent matcher, declaration and projection
contracts. The final 45 rewrites preserve the named tests and current
evidence is linked from the inventory.

The final three assertion gaps are closed. A qualifying repeat summon keeps
Star ownership and individual history intact. Water Dotting Fan distinguishes
an earlier Formation from the immediately previous one in the canonical
Attack status list. Four-Player Shock Burst records both Battle Soul statuses
for each of the three other Players.

Every rewritten case checks event replay and recorded decision verification.
For the same-turn Spirit allowance, the narrow projection assertion applies
the Spirit Summoning event to isolate its reset. The Cannot Act and skill
behavior in that test runs through recorded Commands.

### Existing matrices and narrow contracts

- Keep the exact non-owner `ChoiceMade` projection for a genuine search answer. The fixture isolates the public event payload. [`clear_wind_ten_thousand_miles_kept_cards_are_hidden_from_other_players`](../tests/single/confluence_generation.rs#L515)
- Keep the tier-pattern and unaffiliated route matcher. It asks for `ChangeProfession` candidates at known tiers. [`all_school_transitions_and_unaffiliated_routes_are_queryable`](../tests/single/hero_schools.rs#L229)
- The legal Metal ladder matrix checks each Change Profession Card move, the final observer Profession and both replay paths. Keep the historical JSON round trip as a separate assertion. [`metal_profession_ladder_and_hero_resistance_matrix_use_legal_turns`](../tests/single/hero_warrior_interaction_matrix.rs#L764), [`profession_change_progresses_in_order_and_replays_explicit_card_moves`](../tests/single/hero_warrior.rs#L107)
- The War God Weapon Mastery matrix proves the ordinary Weapon identity and typed draw bonus after legal acquisition. Keep the historical Divine Weapon offer assertion. [`war_god_weapon_mastery_matrix_keeps_weapon_identity_and_adds_turn_draw_through_legal_ladder`](../tests/single/hero_warrior_interaction_matrix.rs#L1152), [`warrior_proficiencies_keep_original_formation_identity_and_effect`](../tests/single/hero_warrior.rs#L224)
- The legal Wood Star and Defense matrix checks the exact substitution, covered Formation and owner/opponent views. Keep the observer redaction assertion. [`owned_star_defense_matrix_records_selected_substitution_and_preserves_star_lifecycle`](../tests/single/star_defense_interaction_matrix.rs#L172), [`substituted_passive_records_and_redacts_the_declared_card`](../tests/single/star_rules.rs#L267)
- Retain both exact substitution offers, the missing-declaration validation error and acceptance of the explicitly selected alternative. [`multiple_star_substitution_options_require_the_selected_card`](../tests/single/star_rules.rs#L331)

## Validation and completion

The latest `cargo test --locked` passed all 805 Rust tests, including 395 in
`single`. All 45 previously queued tests ran in that target.

The browser review paired the existing privacy, reconnect, configuration
and endgame seams with named Rust tests. The Chain and Flying Blade tests
have new browser assertions for reconnect while a choice is pending and
independent Observer reconnect, respectively. All eight inventoried
Playwright scenarios passed against the local Worker with the workspace
toolchain.

For each new matrix, establish effects under test with legal Commands,
provide independently failing baseline, modifier and combined cases, and
check canonical outcomes, affected and unaffected consequences, costs, Card
movement, final state and replay where applicable. Compare multiple
No-Effect Grounds as a set and reject duplicates. Use the
[rules testing checklist](../docs/rule.md#9-testing-checklist).

Removal still requires named passing evidence for every unique assertion and
joint review of the decision. Keep distinct matcher, catalog, serialization,
validation atomicity, public-view and replay contracts. Historic
`cargo test --test <old-suite>` commands in the inventory record earlier runs.
Current focused runs use `cargo test --test single <module>`.
