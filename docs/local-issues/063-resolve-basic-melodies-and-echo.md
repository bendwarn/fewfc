# 63 Resolve basic Melodies and Echo

## Triage

ready-for-agent

## What to build

Add an engine-only Echo Theme Rule Module slice containing 商調‧鳴金,
角調‧落木, 羽調‧流水, 徵調‧戰火, and 宮調‧裂土. Implement typed Echo
policies, optional Echo Cost resolution, Scheduled Echo, fresh Turn Start
main-effect execution, and all five published effects without releasing the
incomplete module to new Online Game Rooms.

## Acceptance criteria

- [x] Echo has one stable Rule Module identity in the Rust catalog, requires Star, Five Directions Legend, and Hero Schools, and remains excluded from default and Web-available modules until all seven Melodies are complete.
- [x] The five basic Melodies use their exact two-Card patterns and remain explicit Formation declarations when the same Cards also match a Base Ruleset Formation.
- [x] Star Element Substitution, Formation-specific Profession Proficiencies, and Sacred Art Multiplicity do not make a Melody legal; only Card Interpretation Layers whose own scope includes that Formation may affect matching.
- [x] Melody definitions use typed Optional Cost, Automatic, or None Echo policies and an execution origin that prevents Echo or 變宮‧植土 from recursively scheduling Echo.
- [x] Optional Echo Cost is offered only after the complete main-effect continuation; the Player may choose one currently held Card with an allowed printed element or explicitly Decline.
- [x] Echo Cost is an ordinary Discard with normal Card Origin and Pile Owner movement; it neither charges a Spirit nor becomes a Retrievable Discard.
- [x] A successfully executed no-change main effect remains Echo-eligible, while Seal, 裂土, or another whole-effect invalidation prevents the original Formation Use from scheduling Echo.
- [x] Scheduled Echo stores only Player, Melody, and due timing; targets, Deck contents, Turn Order targets, and other choices resolve from Game State at that Player's next Turn Start.
- [x] Echo executes only the Melody main effect: it is not a Formation Use, consumes no action or Formation Cards, triggers no Covered Passive, updates no previous Formation, and satisfies no on-perform rule.
- [x] 角調‧落木 restores 15 HP to the performing Player's Team up to initial HP; 徵調‧戰火 removes 15 HP from the Previous Player's Team outside the Attack, Shield, and five-element pipelines.
- [x] 羽調‧流水 stacks public Flow State layers; after ordinary Turn Draw modifiers, at most one layer is spent only when an allowed Turn Draw would otherwise not fill the hand.
- [x] 宮調‧裂土 privately exposes the Next Player's hand to the performer, then accepts any Formation in the enabled Formation Catalog and publicly records the selected Formation, target, and expiry.
- [x] 裂土 makes only a matching Formation Use during the target's next turn ineffective; it does not suppress Echo or 變宮‧植土 main-effect execution.
- [x] An Ineffective Formation still consumes its action and Cards and satisfies independent on-perform rules, but its Formation effects and Echo scheduling do not execute.
- [x] 商調‧鳴金 searches the performing Player's current Personal Deck or the shared Deck, regardless of Card Origin, publicly reveals the selected Card, shuffles the remainder through Pending Randomness, and places the selected Card on top.
- [x] An empty Deck first performs ordinary pile-scoped Discard recycling through trusted randomness before 鳴金 searches it; the post-search remainder then receives its own required shuffle.
- [x] When both the Deck and its recyclable Discard Pile are empty, 鳴金 is a no-change executed main effect rather than a Validation Failure; when only the selected Card remains, it creates no unnecessary post-search randomness request.
- [x] Semantic Echo decline, scheduling, start, and resolution events plus effect deltas reproduce identical direct, replayed, and verified state.
- [x] Focused Rust tests cover both Deck modes, two- and four-Player Turn Order, overlapping Formation declarations, hidden information, invalidation, no-change effects, game-ending War Fire, and every Echo Cost element.

## Validation

- `cargo test` and strict Clippy pass, including 16 focused Echo integration tests.
- Web unit tests (28), Nuxt type checking, and the production build pass.
- Echo remains engine-only and absent from the shared Web Rule Module catalog.
- A fresh Brave E2E run could not start because the execution environment
  rejected the sandbox escalation for quota reasons; the prior 19-test suite
  passed before this engine-only slice.

## Blocked by

- [#62 Generalize choices and midgame randomness](062-generalize-choices-and-midgame-randomness.md)
