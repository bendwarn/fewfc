# 42 Resolve Void Star Breaking atomically

## Triage

ready-for-agent

## What to build

Implement 虛空破星術 as an active Spell available when the Star Rule Module is
enabled and formed from three same-level Card Instances. When it resolves, it
breaks every currently owned Star and directly removes 20 HP from each affected
Team as one atomic Formation Use.

Use the existing Spell, passive-counter, HP delta, Game Outcome, replay, Public
View, and Web Formation paths rather than creating a standalone action.

## Acceptance criteria

- [ ] Three same-level Card Instances match 虛空破星術 only when the Star Rule Module is enabled.
- [ ] 虛空破星術 is an active Spell submitted through the normal Formation Use command.
- [ ] Seal cancels the Spell before any Star is broken or HP is changed.
- [ ] Successful resolution emits one `StarBroken` event for every affected Team Star.
- [ ] Each Team whose Star is broken loses 20 HP directly; Teams without a Star lose no HP.
- [ ] The HP loss does not interact with Player Shields.
- [ ] All Star Breaking and HP changes resolve before Game Outcome evaluation.
- [ ] If every Team reaches zero HP, the result is a draw.
- [ ] Public State View and Public Event Feed show the complete viewer-safe result.
- [ ] Direct execution and replay produce identical Star, HP, and Game Outcome state.

## Blocked by

- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
