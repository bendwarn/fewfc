# 45 Deepen Card selection into playable Actions

## Triage

ready-for-agent

## What to build

Deepen the current Card-selection flow so one rule-backed query returns every
Action candidate that exactly matches the selected Card Instances. Preserve
Formation Uses and add the response shape needed for later Profession Changes
without making the client infer rules or enumerate hand combinations.

Split the battlefield's central controls vertically on every viewport: an
upper Ability panel for Active-Effect Commands that do not close the Main
Phase, and a lower Action panel for Formation Uses and Pass Action. Preserve
the existing interaction while creating the end-to-end seam required by Hero
Schools.

## Acceptance criteria

- [x] Selecting Cards queries one playable-Action interface shared by validation and presentation.
- [x] Existing Formation candidates retain their identity, summary, and submission behavior.
- [x] The candidate schema can distinguish Formation Uses from future Profession Changes.
- [x] The client never chooses Cards or reconstructs legality outside the Rules Engine.
- [x] The Ability panel contains Discard Retrieval and states that successful use does not end the turn action.
- [x] The Action panel contains Formation candidates and Pass Action and states that successful use ends the Main Phase.
- [x] Ability appears above Action at every viewport width; neither panel changes to a side-by-side layout.
- [x] Loading, error, empty-selection, keyboard, focus, and accessible-name behavior remain covered.
- [x] Rust, Web API, component, and Playwright assertions preserve existing Base and Optional Rule behavior.

## Blocked by

None - can start immediately
