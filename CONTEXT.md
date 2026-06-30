# CFECards Rules Engine

Deterministic rules engine for CFECards game state, formation resolution, turn flow, event replay, hidden information, and team-mode combat.

## Language

**Rules Engine**:
A pure Rust library that validates commands, advances deterministic game state, resolves formations, emits canonical events, and supports replay.

**Ruleset**:
A complete deterministic rule module for setup validation, turn constants, formation matching, effect resolution, and event decisions.
_Avoid_: formation registry

**Base Ruleset**:
The first official ruleset implemented by the engine.
_Avoid_: team mode ruleset

**Game Record**:
The canonical persisted game history made from setup and accepted setup, command, and automatic advancement decisions.
_Avoid_: save file, snapshot

**Game State**:
The current projection derived from a **Game Record**.
_Avoid_: record, save

**Game Event**:
A canonical fact emitted by accepted setup, command, or automatic advancement and used for replay.
_Avoid_: log message, notification

**Validation Failure**:
A rejected command or setup because the submitted player/input data is illegal for the current rules and state.
_Avoid_: rule bug

**Rule Implementation Error**:
A failure caused by an incomplete or internally inconsistent rule module after input validation has succeeded.
_Avoid_: validation failure

**Engine Invariant Error**:
A failure caused by an impossible engine state or violated core invariant.
_Avoid_: player error

**Public View**:
A viewer-filtered representation of canonical game data for API or presentation use.
_Avoid_: replay source

**Public State View**:
A viewer-filtered snapshot of game state.
_Avoid_: canonical state

**Public Event Feed**:
A viewer-filtered event stream derived from canonical events.
_Avoid_: canonical event log

**Card Move Delta**:
An explicit recorded movement of a card instance from one zone to another.
_Avoid_: inferred movement

**Discard (捨棄)**:
The action of moving a used or unwanted Card Instance to the Discard Pile.
_Avoid_: 棄置

**Discard Pile (棄牌堆)**:
The public zone containing discarded Card Instances. A card selected under the
Turn Draw discard rule is a **Discarded Card (棄牌)**.
_Avoid_: 捨棄區

**Card Instance**:
A movable individual card in a game zone.
_Avoid_: card definition

**Card Definition**:
Immutable printed-card data such as name, one five-element element, and level from 1 to 5.
_Avoid_: card instance

**Card Back**:
The viewer-safe appearance of a hidden Card Instance. It conveys no element, level, owner, or other game information and may vary cosmetically.
_Avoid_: hidden-card label, element mark

**Player**:
A seat participant in turn order.
_Avoid_: user, account

**Team**:
The HP-owning side that one or more players belong to.
_Avoid_: player HP owner

**Shield (防護罩)**:
A player-owned persistent value that takes damage in place of the owning Player.
Each Player may have at most one Shield.
_Avoid_: 護盾, team shield

**Turn Order**:
The circular player sequence used to decide the current player, previous player, next player, and passive trigger relationships.
_Avoid_: team order

**Previous Player**:
The player immediately before the current player in turn order.
_Avoid_: opponent, enemy

**Previous Turn**:
The immediately completed Player turn before the current turn. It is not the previous round or the Previous Player's older history.
_Avoid_: previous round, previous formation history

**Formation**:
A declared combination of card instances that matches a formation pattern and resolves through an effect definition.
_Avoid_: combo, hand pattern

**Formation Use**:
The accepted use of a declared formation, including its semantic resolution and explicit card movement.
_Avoid_: implicit card discard

**Formation Category**:
The official top-level kind of a formation: attack or spell.
_Avoid_: elemental attack category, active spell category, passive spell category

**Effect Plan**:
The execution plan linked from a formation effect definition, such as attack resolution, immediate spell resolution, or covered passive resolution.
_Avoid_: formation behavior

**Attack Plan**:
The attack execution detail that distinguishes elemental, physical, and special attacks.
_Avoid_: attack category

**Attack**:
A formation category that targets the previous player first and resolves HP impact through that player's team.
_Avoid_: damage spell

**Spell**:
A formation category whose effect may resolve immediately or be covered as a passive.
_Avoid_: effect, skill

**Spell Type**:
The active or passive procedure used to perform a Spell. Spell Type determines
whether submitted cards are shown and resolved immediately or placed face down;
it is distinct from Formation Category and is not copied by class change.
_Avoid_: spell category

**Covered Passive**:
A hidden passive spell placed by a player and checked at the next player's action start.
_Avoid_: trap, secret

**Counter Effect**:
A delayed defensive Formation effect that checks and may modify the next Player's
action. Passive Spells create hidden Counter Effects through Covered Passives;
class change may create the copied Counter Effect publicly without covered cards.
_Avoid_: covered passive effect

**Resolved Formation Effect**:
The Formation Category and effect behavior actually adopted by a Formation Use,
stored separately from the performed Formation's identity. Class change keeps its
own Formation identity while copying a previous Resolved Formation Effect.
_Avoid_: displayed formation, formation id

**Status Effect**:
A rule-recognized ongoing effect attached to a player or team.
_Avoid_: arbitrary tag

**Status Kind**:
The rule-recognized type of a status effect.
_Avoid_: string metadata

**Cannot Act**:
A status kind that prevents a player from taking an action command.
_Avoid_: stunned, disabled

**Main Phase**:
The public input phase where the current player may use active-effect commands before exactly one action command closes the phase.
_Avoid_: active window, action phase

**Turn Draw (回合抽牌)**:
The mandatory draw step in the turn flow. It draws one extra card and requires
the Player to choose one of those cards to Discard.
_Avoid_: 抽牌選擇

**Active-Effect Command**:
A non-formation player ability command that may be used during the main phase without closing the player's action opportunity.
_Avoid_: formation action

**Action Command**:
The command that consumes the player's action opportunity and closes the main phase.
_Avoid_: active-effect command

**Pending Choice**:
A serialized waiting state requiring a player decision before deterministic resolution can continue.
_Avoid_: prompt, callback

**Choice Requested**:
A game event moment that creates a pending choice for one player.
_Avoid_: UI prompt

**Choice Made**:
A game event moment that records the selected answer to a pending choice.
_Avoid_: callback response

## Relationships

- A **Player** belongs to exactly one **Team**
- A **Team** owns HP for one or more **Players**
- A **Player** owns at most one **Shield**
- A **Game Record** uses exactly one **Ruleset**
- The **Base Ruleset** supports both two-player and team-mode setup shapes
- **Turn Order** is player-based, not team-based
- The **Previous Player** is derived from **Turn Order**
- A **Formation** has exactly one **Formation Category**
- A **Formation** resolves through exactly one **Effect Plan**
- A **Spell** has one **Spell Type** that controls its performance procedure
- A **Formation Use** retains its Formation identity separately from its **Resolved Formation Effect**
- A **Formation Use** records zone changes through **Card Move Deltas** or equivalent replayable deltas
- A **Card Move Delta** moves one **Card Instance**
- **Discard** moves a **Card Instance** to the **Discard Pile**
- A **Card Instance** refers to one **Card Definition**
- A **Card Definition** has exactly one element: metal, wood, water, fire, or earth
- A **Card Definition** has a level from 1 to 5
- An **Attack Plan** may be elemental, physical, or special without changing the **Formation Category**
- An **Attack** targets a **Previous Player** before resolving HP impact to that player's **Team**
- The **Main Phase** may accept multiple **Active-Effect Commands** before one **Action Command**
- A successful **Action Command** closes the **Main Phase**
- A **Covered Passive** belongs to one **Player** and flips at the next player's action start
- A **Counter Effect** may be hidden behind a **Covered Passive** or publicly established by class change
- A **Status Effect** has one **Status Kind**
- **Cannot Act** allows an action pass for the affected **Player**
- A **Choice Requested** creates one **Pending Choice**
- A **Choice Made** answers one **Pending Choice**
- A **Game Record** projects **Game Events** into **Game State**
- A **Public View** is derived from canonical data and is not used for replay
- A **Public Event Feed** is derived from **Game Events**
- A **Public State View** is derived from **Game State**
- A **Rule Implementation Error** emits no **Game Events** and does not mutate **Game State**
- A **Validation Failure** emits no **Game Events** and does not mutate **Game State**

## Example Dialogue

> **Dev:** "When a player performs an attack, do they choose an opponent?"
> **Domain expert:** "No. The attack targets the **Previous Player** from **Turn Order**. HP damage then applies to that player's **Team**."

## Flagged Ambiguities

- "opponent" is too vague in team mode; use **Previous Player** when the rule follows seating order, and **Team** only when describing HP ownership.
- Elemental, physical, special, immediate, and passive are execution details, not **Formation Category** values; keep **Formation Category** to **Attack** or **Spell**.
- Active-effect commands are not formation actions; do not treat every command accepted during **Main Phase** as an **Action Command**.
- The first version of the base formation engine may define **Active-Effect Command** as a future extension point without implementing concrete commands.
- `used_cards` identifies cards used by a **Formation Use**, but card zone changes must still be represented by **Card Move Deltas** or equivalent explicit replayable deltas.
- Turn draw discard and effect-generated selections are both **Pending Choices**, even when events keep more specific semantic names.
- `CannotAct` is the implementation spelling of **Cannot Act**, a canonical **Status Kind**, not arbitrary metadata.
- Public event data is a **Public Event Feed**, not a replayable **Game Event** log.
- A known formation with a legal declaration but missing resolver is a **Rule Implementation Error**, not a **Validation Failure**.
- "card" is ambiguous; use **Card Instance** for zone membership and **Card Definition** for immutable printed-card data.
- Team mode is a setup shape supported by the **Base Ruleset**, not a separate **Ruleset** unless future rule behavior diverges.
