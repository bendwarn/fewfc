---
status: accepted
---

# Bind games to rule versions with complete theme sets

Use the full Rule Version, initially 5.16 and 5.17, to identify applicable rule
content, including the behavior of same-named rules, and its complete Theme
Rule Module set. Selecting a version automatically enables every included
Theme Rule Module; themes are not individually selectable or combinable across
versions. Both versions remain available for new games, and each game retains
its selected version through system upgrades so later releases do not silently
change an ongoing game's rules.

Version 5.16 includes Spirit, Jianghu, Echo, Confluence Generation, Tribulation,
Pouch, and Dark Glimmer. Version 5.17 includes Spirit, Jianghu, Confluence
Generation, Pouch, Dark Glimmer, and Totem Formations (圖騰法陣).

This decision supersedes ADR-0001's independently configurable themes and its
deferral of rule version identity, plus section 39 of
`docs/rules-engine-decisions.md` where it permits independent theme selection
and defaults to every module in the global catalog. It does not decide record
schema compatibility or migration, which are distinct from Rule Version.

## Remaining interview decisions

The accepted scope above is not an implementation-complete specification.
Basic/advanced-only play, optional-rule dependencies, existing unversioned
records, product-specific rulings and future corrections, and delivery scope
remain to be resolved during the interview.
