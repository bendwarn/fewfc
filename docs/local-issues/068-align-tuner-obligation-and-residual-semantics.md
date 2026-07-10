# 68 Align Tuner obligation and residual semantics

## Context

The Confluence Generation Tuner profession system currently diverges from the
confirmed domain model around Residual Element/Level lifecycle, 調律 obligation
reachability, formation role distinctness, and resonance continuation ordering.

## Scope

- Persist Residual Element and Residual Level as turn-scoped facts independent
  of whether the source Card remains retrievable.
- Keep 調律 and 天響 dependent on the physical Retrievable Discard, while
  Profession Changes and Formations read the residual facts.
- Enforce 調律 as an authoritative rules-engine obligation:
  - offer 調律 only when at least one legal completion path exists;
  - hide/reject voluntary options that consume the 調律牌 or remove every
    remaining completion path;
  - require the 調律牌 in an allowed Profession Change or, under 易弦, an
    available inherited Profession Formation.
- Require distinct physical Card Instances for distinct Formation roles,
  including the five-resonance element and residual-level slots.
- Resolve 千鳴 and 萬鳴 in the confirmed order and defer Game Outcome until
  all pending resonance choices complete.
- Treat 煌鳴 affected players and 森鳴/萬鳴 recovery restrictions according to
  the confirmed affected-player and performer-scoped semantics.

## Acceptance

- [x] Residual Element/Level remain usable after the source Card moves, until
      the current Player reaches Turn Draw.
- [x] 調律/天響 require the physical Retrievable Discard to still be available.
- [x] Five-resonance Formations reject a single Card satisfying both required
      slots.
- [x] 易弦 allows the 調律牌 in all currently available inherited Profession
      Formations.
- [x] Commands and playable action queries cannot strand an active 調律牌
      obligation.
- [x] 千鳴/萬鳴 continuation and outcome timing match the decision document.
- [x] 煌鳴 direct affected-player behavior and 森鳴/萬鳴 recovery restriction
      behavior are covered by tests.

## References

- `CONTEXT.md`
- `docs/rules-engine-decisions.md`
- `docs/adr/0022-validate-tuning-obligations-by-reachable-actions.md`
