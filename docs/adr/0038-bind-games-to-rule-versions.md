---
status: accepted
---

# Bind games to rule versions independently of enabled modules

Use the full Rule Version, initially 5.16 and 5.17, to identify applicable rule
content, including the behavior of same-named rules, and available Theme
Rule Modules. Rule Version and enabled modules are separate: a game using only
some 5.16 themes still runs 5.16. Themes cannot be combined across versions.
Both versions remain available for new games, and each game retains
its selected version through system upgrades. Deliberate rule changes require
a new Rule Version; corrections to the implementation of the selected rules
follow the bug-fix policy below.

Version 5.16 includes Spirit, Jianghu, Echo, Confluence Generation, Tribulation,
Pouch, and Dark Glimmer. Version 5.17 includes Spirit, Jianghu, Confluence
Generation, Pouch, Dark Glimmer, and Totem Formations (圖騰法陣).

Existing unversioned games belong to 5.16 and retain their stored enabled
modules; do not classify them as a separate legacy rule version or add modules
to their running games. Persistence changes must preserve that interpretation
without rewriting their recorded outcomes.

Room creation keeps its existing interface, adding a collapsible list for
selecting past Rule Versions. New rooms default to 5.17. Selecting or changing
the Rule Version initially enables all themes included in that version; players
may then deselect themes using the existing module controls. Themes absent
from the version are unavailable, and the existing dependency constraints still
apply. This replaces a global-catalog default without removing partial-module
play.

Only the room owner may change the Rule Version before game start. A version
change resets other players' readiness and invalidates locked waiting-room deck
snapshots, following the existing configuration-change policy in ADR-0004.
Once the game starts, its Rule Version cannot change.

Both versions retain existing product rulings, including Temporary Ability Loss
for 離山 (ADR-0033) and Pouch's Personal Deck and Spirit dependencies (ADR-0021).
Selecting all themes therefore also requires Personal Deck; deselecting themes
continues to follow the existing dependency behavior. Rule Version identifies
this product's interpretation of the published version, not an assertion of
verbatim equivalence to the PDF.

The first delivery includes full 5.17 support, including Totem Formations and
its interactions, version selection, persistence, and replay support, alongside
continued 5.16 support. The version selector must only offer playable versions.

This decision supersedes ADR-0001's deferral of rule version identity and
section 39 of `docs/rules-engine-decisions.md` where defaults use every module
in the global catalog: available themes now come from the selected version.
Record schema compatibility is distinct from Rule Version.

## Bug fixes and historical records

Bug fixes apply to subsequent operations under the affected Rule Version,
including ongoing games. Previously recorded outcomes are not recomputed:
replay applies the original canonical events, as specified in section 33 of
`docs/rules-engine-decisions.md`. Deliberate balance or rule changes require a
new Rule Version. This delivery does not preserve every historical engine build
to reproduce and verify the behavior of past implementation bugs.

The version-mechanism decisions are settled. The interview continues with
Totem Formation semantics and interactions before implementation begins.
