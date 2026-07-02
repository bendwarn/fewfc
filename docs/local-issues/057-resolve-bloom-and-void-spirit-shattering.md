# 57 Resolve Bloom and Void Spirit-Shattering atomically

## Triage

ready-for-agent

## What to build

Complete the Spirit Theme Rule Module with automatic Bloom and Void
Spirit-Shattering Technique. Resolve simultaneous Spirit and Team changes
without transient winners, preserve direct-victory priority, and make direct
execution, replay, reconnect, event history, and final online outcomes agree.

## Acceptance criteria

- [x] A six-power Wood Spirit automatically uses Bloom when its Team reaches zero HP from an HP-based resolution.
- [x] Every eligible Wood Spirit on that Team consumes six power and contributes 40 recovery in one atomic resolution before HP-based Game Outcome evaluation.
- [x] Combined Bloom recovery is capped at initial Team HP, and Spirits that reach zero power break.
- [x] Five-Star Alignment and other direct victories finish without triggering automatic Bloom.
- [x] 虛空碎靈術 is a three-same-level Active Spell available only with Spirit enabled and remains subject to the ordinary Active Spell pipeline.
- [x] Void Spirit-Shattering atomically reduces every Spirit's power by two before directly removing 20 Team HP for each Spirit owner.
- [x] Multiple Spirit owners on one Team each contribute their 20-point HP loss, and all affected Teams resolve before Game Outcome evaluation.
- [x] Spirit Breaking and automatic Bloom observe post-shattering power, so a Wood Spirit reduced from six to four cannot answer HP loss from the same Technique.
- [x] Canonical composite events contain sufficient Spirit Power, Spirit Breaking, Team HP, and Card movement deltas for replay without recomputing rules.
- [x] Direct execution, replay, recorded-decision verification, Public Views, online history, reconnect, Draw outcomes, and winner presentation agree.
- [x] Focused Rust, Web unit, and self-contained Brave Playwright tests cover simultaneous teammate Bloom, HP caps, direct victory, mixed Spirit power, multiple owners per Team, both Teams reaching zero, Seal, and replay.

## Blocked by

- [#55 Summon and display Spirits](055-summon-and-display-spirits.md)
- [#56 Use Spirit Skills](056-use-spirit-skills.md)
