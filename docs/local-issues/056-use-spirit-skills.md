# 56 Use Spirit Skills

## Triage

ready-for-agent

## What to build

Let Players use every Metal, Wood, Water, Fire, and Earth Spirit Skill as a
complete active-effect path. Preserve hidden hand information, compose
turn-scoped Card Interpretation Layers across Star and Hero Schools, and make
skill legality, semantic history, playable actions, reconnect, and the online
Ability panel agree.

## Acceptance criteria

- [x] Spirit Skills are typed active-effect commands that do not close the Action opportunity and remain legal while the Player has Cannot Act.
- [x] Each individual Spirit may successfully use one Skill per Player turn; replacing a Spirit provides the new Spirit its own allowance.
- [x] Validation checks printed timing, input, and Spirit Power requirements but does not require a beneficial result; failure emits no events and consumes no power.
- [x] 飛刃、劍雨、芬芳、manual 綻放、川流、浩瀚、螢光、絢爛、石盾、and 岩壁 implement their official costs and effects.
- [x] Metal Skill HP loss bypasses Shields and Attack modifiers; Wood recovery respects initial Team HP; Water card movement respects Card Origin and Personal Deck piles.
- [x] Fire Skills create turn-scoped Card Interpretation Layers whose later values replace only the dimensions they specify and expire at Turn End.
- [x] Fire interpretation affects every level read in the turn, including Formation matching and points, Profession requirements and costs, and Star qualification.
- [x] Fire's selected Card remains visible only to its owner until ordinary Card movement reveals it; the Skill and declared level remain public.
- [x] Sacred Art Multiplicity may compose with Fire level interpretation but never with Star Element Substitution; both match slots share one element and level and cannot be reinterpreted independently.
- [x] Stone Shield prevents only the next Player's attack damage during that Player's next turn, then expires; it prevents Sacred Beast damage without cancelling other Attack effects.
- [x] Spirit kind and power remain in Public State while successful Skill uses are represented by Public Event Feed history rather than a duplicate presentation flag.
- [x] Direct execution, replay, recorded-decision verification, playable actions, Web commands, event presentation, and reconnect agree for every Skill.
- [x] Focused Rust, Web unit, and Brave Playwright tests cover all ten Skills, invalid and ineffective uses, Cannot Act, replacement, hidden Fire targets, cross-module interpretations, Personal Deck, and Sacred Beast interaction.

## Blocked by

- [#55 Summon and display Spirits](055-summon-and-display-spirits.md)
