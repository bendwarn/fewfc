# 23 Complete passive action modifications and sealed passives

## Type

AFK

## Parent

Derived from `docs/rules-engine-decisions.md` section 25 and closed GitHub issues #17, #18, #21.

## What to build

Finish passive spell resolution beyond flip/no-effect bookkeeping. Passive effects should return action modifications that the incoming action resolution applies deterministically. Defense should affect incoming attacks, seal should affect incoming spells, and a sealed covered passive should later flip as no effect without revealing additional public information early.

## Acceptance criteria

- [ ] Passive resolution produces action modifications rather than only `PassiveFlipOutcome::Applied`.
- [ ] `defense` changes incoming attack resolution according to the selected first-version rule behavior and records replayable outcome data.
- [ ] `seal` applies only to incoming spells.
- [ ] When `seal` applies to an incoming passive cover action, the incoming covered passive remains covered and is marked sealed internally.
- [ ] A sealed covered passive later flips at its own trigger timing, resolves as no effect, and is discarded normally.
- [ ] Public/player views do not expose the sealed marker beyond normal covered-card visibility rules.
- [ ] Tests cover defense against attack, seal against active spell, seal against passive cover, sealed passive later no-effect discard, and replay consistency.

## Blocked by

- #25

