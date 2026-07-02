# 55 Summon and display Spirits

## Triage

ready-for-agent

## What to build

Add the first complete Spirit Theme Rule Module slice. New official rooms enable
Spirit by default together with its required Advanced Rule Modules. Players can
summon any of the five elemental Spirits, replace their current Spirit, charge
it through the matching Turn Draw discard, and see every Player's public Spirit
and Spirit Power through direct play, replay, reconnect, and the online room UI.

## Acceptance criteria

- [x] Spirit is an official Theme Rule Module with one stable identity shared by setup, Game State, persistence, Web DTOs, room metadata, and UI.
- [x] Spirit setup is valid only when Star, Five Directions Legend, and Hero Schools are all enabled; invalid combinations fail before events or mutation.
- [x] New rooms enable every available Rule Module, including Spirit, while existing stored rooms retain their module list.
- [x] Enabling Spirit in a waiting room enables all three dependencies; disabling any dependency disables Spirit; the server rejects bypassed invalid combinations.
- [x] Rule Module changes preserve the existing readiness and Locked Deck List invalidation behavior.
- [x] 金靈喚術、木靈喚術、水靈喚術、火靈喚術、and 土靈喚術 are exact two-Card Active Spells available only with Spirit enabled.
- [x] Successful Spirit Summoning gives the Player the declared Spirit at two Spirit Power and replaces any previously owned Spirit, including the same kind.
- [x] Each Player owns at most one Spirit, while different Players may own the same Spirit kind.
- [x] Choosing a matching-element Turn Draw discard adds one Spirit Power only to the discarding Player's Spirit and caps it at six.
- [x] Canonical Spirit Summoning and Spirit Power events reproduce identical state through direct execution, replay, and recorded-decision verification.
- [x] Public State exposes every Spirit's owner, kind, and current power; the online room displays that information without an `already used` presentation field.
- [x] Focused Rust, Web unit, and self-contained Brave Playwright tests cover module dependencies, default-on rooms, disabled Spirit, replacement, charging, reconnect, and two-Player and four-Player ownership.

## Blocked by

None - can start immediately
