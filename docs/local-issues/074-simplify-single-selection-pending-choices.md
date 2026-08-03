# 74 Simplify single-selection Pending Choices

## Triage

ready-for-agent

## Goal

Remove redundant confirmation from single-selection Pending Choices while
keeping every submitted outcome clear from its control or semantic guidance.

## Problem Statement

Every Card Pending Choice currently uses one toggleable browser draft, selected
count, and confirmation button even when its maximum is one. The Player must
therefore click the only answer they intend and then confirm the same intent.
Player, Formation, and Environment Pending Choices already submit with one
click, so the extra Card step is inconsistent as well as redundant.

The generic Card draft also exposes the canonical zero-or-one encoding of Clear
Wind instead of the decision's player-facing meaning. Its current presentation
makes selecting the revealed Card mean discard, even though selecting a visible
Card more naturally means wanting to keep it. Finally, the Chaos prompt says
Cards return to a hand even though the rule and resolver return them to the top
of the unqualified Deck.

## Solution

Treat a Card Pending Choice with a maximum of one as an immediate interaction,
not a local selection draft. One explicit click constructs and submits the typed
answer. Keep multi-selection drafts and confirmation unchanged. Adapt Clear Wind
through its semantic outcomes while preserving its canonical typed answer, and
correct player-facing wording without changing rules, replay, or server DTOs.

## Confirmed Scope

- Apply this interaction policy only to canonical Pending Choices.
- Do not change Initial Pouch Selection, Main Phase Action Input Requirements,
  Virtual Formation Card selection, direct Secret Strategy input, or other
  selection workflows.
- Preserve canonical Choice IDs, typed answers, validation, continuations,
  lifecycle events, replay, and rule consequences. This is a Web interaction
  and presentation change.

## Current Inventory

The following reachable Card Pending Choices always require exactly one Card:

- Turn Draw Discard
- Holy Wind
- Revelation
- Azure Cloud Step
- Mirror Resonance
- Myriad Resonance
- Thousand Resonance
- Ringing Metal Deck search
- Echo Cost for Ringing Metal, Falling Wood, Flowing Water, War Fire, and Split
  Earth; these choices also permit the separate Decline answer
- Earth Rending Card

Chaos dynamically requires exactly one Card when the target has only one Card,
and exactly two Cards when the target has at least two.

Player, Formation, and Environment Pending Choices already submit immediately
when an option is selected. Their reachable cases are Pure Fire target, Split
Earth Formation, Plant Earth Melody, and Earth Rending Environment.

Clear Wind exposes a zero-to-one Card range but is nevertheless a
single-selection decision: the Player chooses between discarding the revealed
Card and returning it to the Deck top.
Its canonical zero-or-one Cards answer must not force the Web interface to
present a generic Card-selection draft. Clear Wind Ten Thousand Miles cannot
reach a one-Card maximum in legal play: performing its four-Card Formation
frees at least four of the five hand slots before the keep choice is requested.
It remains a multi-selection choice. Chain and Sheep Stealing are composite
choices. The `metamorphosis` and `sealCard` Web presentation variants have no
reachable typed Choice Continuation.

## User Stories

1. As a Player answering a single-Card Pending Choice, I want one Card click to
   submit my answer, so that I do not confirm the same intent twice.
2. As a Player answering a multi-Card Pending Choice, I want to review and
   change my draft before confirmation, so that removing single-choice
   confirmation does not weaken multi-choice control.
3. As a Player using Clear Wind, I want clicking the revealed Card to mean
   keeping it on top of the Deck, and an explicit discard action to mean I do
   not want it.
4. As a Player considering Echo, I want either an eligible cost Card or
   `放棄迴響` to be a complete immediate answer.
5. As a reconnecting Player, I want the current choice to wait for my explicit
   click even when it has only one legal answer, so that rendering a view never
   performs a command.
6. As a Player whose immediate answer fails to submit, I want the same choice
   restored with an error and retryable controls, so that failure does not look
   like acceptance.
7. As a Player answering Chaos, I want its prompt to use the official Deck-top
   wording, so that the displayed consequence matches the rule.

## Confirmed Interaction Policy

- A Pending Choice's typed canonical answer shape does not dictate its visual
  control. In particular, a Card answer need not use the generic Card-selection
  interface.
- Classify Clear Wind by its two mutually exclusive rule outcomes, not by its
  zero-or-one Cards encoding. Present it as a single-selection decision and do
  not require a separate confirmation step.
- Preserve the existing canonical Clear Wind encoding while mapping it to the
  Player's intent in the Web interaction: clicking the revealed Card means
  keeping it on the Deck top and submits the empty Cards answer; clicking the
  `捨棄此牌` button means rejecting it and submits that Card in the Cards answer.
- Explain the Clear Wind Card control as `選擇此牌則放回牌堆最上方`. Use the
  explicit `捨棄此牌` label instead of an ambiguous `取消` label.
- Treat Echo Cost as a single-selection decision even though its Card choice
  permits Decline. Selecting an eligible Card immediately submits payment;
  selecting `放棄迴響` immediately submits the Decline answer. Decline is an
  alternative outcome, not confirmation of a Card draft.
- For Card choices, use `maximum == 1` as the structural immediate-submission
  boundary. `minimum == 1` has no empty-answer control; `minimum == 0` exposes
  the semantic empty-answer action as well. Clear Wind supplies the only
  currently reachable zero-to-one case and uses its dedicated mapping above.
- Do not add generic `選擇後立即送出` guidance. The absence of a confirmation
  button is sufficient interaction feedback. Keep semantic explanation only
  where the option's result would otherwise be unclear, as with Clear Wind.
- Never answer a Pending Choice merely because the view mounted, reconnected,
  or exposes only one legal result. Require one explicit Player click; remove
  only the redundant second confirmation gesture.
- Do not create or display a local Card-selection draft when `maximum == 1`.
  Hide the selected-count text and confirmation button; each Card is an
  immediate action control. Choices with `maximum > 1` retain toggleable draft
  selection, selected-count feedback, and explicit confirmation.
- Render multi-selection bounds with `~` rather than an en dash, for example
  `已選 1（0~4）`.
- Correct the existing Chaos presentation: selected Cards return from the next
  Player's hand to the top of the unqualified Deck, not to their hand. Do not
  switch between one-Card and two-Card prose at runtime. Follow the official
  rule wording and the glossary's unqualified Deck convention with the fixed
  label `混沌：選擇手牌放回牌堆最上方`.
- Disable every control for the active Pending Choice as soon as an immediate
  answer submission starts. On success, let refreshed canonical state replace
  the choice. On failure, keep the same choice visible, show the existing error
  presentation, and re-enable its controls for an explicit retry. Do not retain
  a single-selection draft or retry automatically.

## Implementation Decisions

- Put the immediate-versus-draft boundary in the Web Pending Choice Interaction
  module rather than enumerating rule continuations in the room page.
- Route both ordinary Game Card controls and the Ringing Metal Card Choice Matrix
  through the same Card-choice interaction entry point. The visual control does
  not decide answer legality or submission timing.
- Build a one-Card typed answer directly when `maximum == 1`; do not first write
  the Card to `selectedChoiceCards` or call the multi-choice submit action.
- Keep the existing draft toggle and `submitPendingChoice` path authoritative
  when `maximum > 1`.
- Detect Clear Wind from its typed public presentation and map its two controls
  to the existing empty and one-Card answer shapes. Do not change its canonical
  Choice Continuation or resolver.
- Reuse the existing `isLoading`, room-connection, Choice ID, command
  transaction, and error-presentation boundaries. Do not add a second
  idempotency or retry mechanism in the interaction helper.
- Keep Player, Formation, Environment, Decline, Chain, and Sheep Stealing answer
  contracts unchanged.

## Testing Decisions

- Add Web Pending Choice Interaction tests proving that `maximum == 1` builds an
  immediate answer while `maximum > 1` only changes the local draft.
- Add focused Clear Wind tests proving that clicking the revealed Card submits
  an empty Cards answer and `捨棄此牌` submits that Card.
- Cover Echo Cost payment and Decline as two immediate typed answers, including
  a state with no eligible cost Card where the Player must still explicitly
  click `放棄迴響`.
- Test that mounting, reconnecting, or exposing one legal answer never submits
  without a Player click.
- Test that an in-flight answer disables the choice, success replaces it from
  canonical state, and failure re-enables the same choice without a selected
  draft or automatic retry.
- Update presentation assertions for Clear Wind, the fixed Chaos rule wording,
  and multi-selection bounds rendered with `~`.
- Update the representative ordinary Game Card E2E flow for Turn Draw Discard
  to prove that one click sends the command and no confirmation button exists.
- Cover the distinct Card Choice Matrix flow through Ringing Metal with the same
  one-click submission assertion.
- Do not duplicate E2E flows for Holy Wind, Revelation, Azure Cloud Step,
  Resonance, Echo Cost, or Earth Rending Card; their immediate behavior is
  already covered by the shared structural interaction tests.
- Validate with focused Web tests, relevant Playwright tests, type checking,
  full `cargo test`, formatting, and diff checks.

## Out of Scope

- Changing any canonical Pending Choice kind, Choice ID, Choice Continuation,
  typed answer, event, rule consequence, replay behavior, or Web DTO.
- Applying immediate submission to Initial Pouch Selection, Main Phase Action
  Input Requirements, direct Secret Strategy inputs, Chain, Sheep Stealing, or
  any workflow outside canonical Pending Choices.
- Removing confirmation from Card Pending Choices whose maximum exceeds one.
- Automatically answering a choice on mount, reconnect, or because only one
  legal result exists.
- Persisting, synchronizing, or restoring single-selection drafts.
- Removing unreachable `metamorphosis` or `sealCard` presentation variants.

## Governing Context

- ADR-0015 defines closed typed Pending Choice answers.
- ADR-0026 and completed issue #70 define the canonical Pending Choice lifecycle
  and keep browser drafts and player-facing presentation in the Web interaction
  module.
- No ADR is proposed for this change: the interaction policy is easy to reverse,
  visible in the UI implementation, and does not alter the canonical domain
  model.

## Blocked by

- none
