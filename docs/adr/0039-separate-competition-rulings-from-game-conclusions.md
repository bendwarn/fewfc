---
status: accepted
---

# Preserve game conclusions alongside competition rulings

Competitive Play uses the existing Ruleset, while Tournament progression may
require a winner for a Game whose canonical conclusion is Draw. Preserve that
Game Conclusion and record the Competition Ruling separately, including its
reason and supporting time-control facts. Never rewrite the original Draw or
append Game Events after GameEnded to represent tournament adjudication.

When both Teams' byoyomi deadlines are the same in the authoritative monotonic
timeline, record a distinct DoubleLoss competition ruling: neither Team is a
winner and the original Game Conclusion remains unchanged. A delayed observation
that crosses two different deadlines records only the Team whose deadline came
first. Tournament scheduling preserves the resulting match as `DoubleLoss`
rather than inventing a winner; its stage progression is governed by the product
design rather than the bracket provider.

This deliberate split keeps replay faithful while allowing elimination
progression and auditable referee rulings; treating the bracket provider's
result as the Game Outcome would erase that distinction.

See [the confirmed product design](../tournament/competitive-play-design.md) for
scope, time controls and progression policy. Provider integration and
the concrete persistence format are not selected by this ADR.
