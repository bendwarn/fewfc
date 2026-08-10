> 規則依據：官方「遊戲規則（完整規則書）」中的回合流程、施展陣法步驟、目標定義、被動術式（蓋牌）觸發點、以及基礎規則陣法列表。([cfecards.org][1])

---

# CFECards Rules Engine Reference

This document is a rulebook-oriented reference for the Rust rules engine. Domain language is defined in [`../CONTEXT.md`](../CONTEXT.md), and implementation decisions are recorded in [`rules-engine-decisions.md`](./rules-engine-decisions.md).

When this document describes implementation shape, it follows those two documents.

Rule statements in this document must be directly traceable to a cited official
rule clause. Do not restate an implementation inference as though it appeared in
the official text. Necessary interpretations belong in
[`rules-engine-decisions.md`](./rules-engine-decisions.md), explicitly labeled
with their source basis and as an implementation interpretation. An ambiguous
case must be confirmed before either document narrows or broadens the source.

## 0) Goal

Build a deterministic CFECards rules engine that:

- Enforces the official fixed turn flow with a canonical phase model:
  `TurnStart -> ActiveEffects -> Action -> TurnDraw -> TurnEnd`.
- Supports `ActiveEffects` as the input phase where a Player may use zero or
  more Active-Effect Commands before one Action Command begins `Action`.
- Supports action commands such as `PerformFormation` and `PassAction`; future rulesets may add more action commands.
- Treats Metamorphosis (`幻化`) as a formation use, not as a standalone `ChangeClass` command.
- Implements formation resolution through effect plans:
  - attack resolution
  - immediate spell resolution
  - covered passive resolution
- Supports both:
  - two-player setup, where each player belongs to their own single-player team
  - team-mode setup, where players are split into two teams and seated alternately
- Persists canonical event logs for deterministic replay, with snapshots as optional cache data.

## 1) Canonical Concepts

### 1.1 Turn And Phase

Each player turn eventually gives control to the next player (`下家`) in circular turn order.

Canonical phases:

1. `TurnStart` - start-of-turn timing and status expiration.
2. `ActiveEffects` - accepts zero or more Active-Effect Commands and one Action
   Command.
3. `Action` - carries the committed Action through every required rule step and
   Pending Choice.
4. `TurnDraw` - draws into the Turn Draw Pool and remains active while the
   discard Pending Choice is unresolved.
5. `TurnEnd` - end-of-turn timing and status expiration.

The engine does not require or event-log an `EndActiveEffects` command. The
first accepted Action Command ends `ActiveEffects` and enters `Action`. Neither
an effect-generated Pending Choice nor the Turn Draw discard choice creates a
new phase; each suspends its owning phase until answered.

Turn Draw follows official rule 4-2.4d exactly:

1. Determine the allowed hand increase `N` from the current hand, hand limit,
   base draw, and applicable modifiers.
2. Draw `N + 1` Cards from the applicable Deck into the single game-scoped
   Turn Draw Pool. These Cards are not yet in the Player's hand.
3. Request one Pending Choice whose legal discards are exactly the Cards in the
   Turn Draw Pool.
4. Resolve one atomic `TurnDrawResolved` fact that explicitly records the
   chosen `discard` and all `kept_cards`. Its rule meaning is that the chosen
   Card is Discarded first and only then do the kept Cards enter the hand; no
   Player input or replay state exists between those movements.

If Turn Draw is skipped because the hand is full or the Player cannot draw, no
Turn Draw Pool or discard Pending Choice is created.

### 1.2 Seating And Targets

- `上家` is the previous player in turn order.
- `下家` is the next player in turn order.
- Turn order is player-based, not team-based.
- Attack base damage targets the previous player first, then HP changes resolve to that player's team.
- `方` targets such as `我方` and `對方` are rule-derived team targets, not player-submitted guesses.
- `declared_targets` stores only concrete choices required by a formation/effect, such as a player, team, or card instance.

### 1.3 Players, Teams, And HP

Use one HP model for both two-player and team mode:

- every player belongs to exactly one team
- HP is owned by teams
- two-player mode uses one single-player team per player
- team mode uses exactly two teams with alternating seating

Do not introduce separate player HP for the base ruleset.

### 1.4 Formation Categories And Effect Plans

Formation category has only two official top-level values:

- `Attack`
- `Spell`

Do not model elemental attack, physical attack, special attack, active spell, passive spell, or Metamorphosis as separate formation categories.

Execution details belong to effect plans:

- an attack effect plan may be elemental, physical, or special
- a spell effect plan may resolve immediately or be covered as a passive
- Metamorphosis (`幻化`) is a basic formation handled through the normal `PerformFormation` pipeline

### 1.5 Card Instances And Card Definitions

Zones store card instances, not card definitions.

- `CardInstanceId` identifies one movable card in the match.
- `CardDefId` identifies immutable printed-card data.
- `CardDef` contains stable printed-card data such as name, element, and level.

Rules resolve `CardInstanceId -> CardDefId -> CardDef` when matching formations or computing effects.

Every card instance also has one immutable Card Origin:

- `Shared` when all players use the shared deck
- `Player(PlayerId)` when the card originated in that player's Personal Deck

Card movement changes the current zone, never Card Origin. This distinction is
required when another player's card temporarily enters the current player's
Deck or hand. Each Deck and Discard Pile separately has a Pile Owner (`Shared`
or `Player(PlayerId)`).

## 2) State Model

Minimum shape:

```rust
enum Phase {
    TurnStart,
    ActiveEffects,
    Action,
    TurnDraw,
    TurnEnd,
}

struct PlayerFormationArea {
    player: PlayerId,
    formation: Option<FormationInArea>,
}

enum FormationAreaState {
    FaceUpResolving,
    FaceDownResolving,
    FaceDownWaiting { sealed: bool },
}

struct GameSetup {
    ruleset: RulesetId,
    enabled_rule_modules: Vec<RuleModuleId>,
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    hp: Vec<TeamHp>,
    card_defs: Vec<CardDef>,
    card_instances: Vec<CardInstanceDef>,
    deck_lists: Vec<PlayerDeckList>,
    hand_limit: usize,
    base_draw: usize,
}

struct GameState {
    status: GameStatus,
    turn_number: u64,
    phase: Phase,
    current_turn_index: usize,
    enabled_rule_modules: Vec<RuleModuleId>,
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    hp: Vec<TeamHp>,
    initial_hp: Vec<TeamHp>,
    decks: Vec<CardPile>,
    hands: Vec<PlayerHand>,
    discard_piles: Vec<CardPile>,
    exposed_foreign_cards: Vec<CardInstanceId>,
    formation_areas: Vec<PlayerFormationArea>,
    turn_draw_pool: Vec<CardInstanceId>,
    pending_choice: Option<PendingChoice>,
    shields: Vec<PlayerShield>,
    counter_effects: Vec<CounterEffect>,
    statuses: Vec<StatusEffect>,
    last_formation_by_player: HashMap<PlayerId, LastFormationUse>,
}
```

Notes:

- `pile.cards[0]` is the top of a Deck.
- Shared-deck games have one shared Deck and Discard Pile.
- Personal Deck games have one Deck and Discard Pile per Player.
- The base hand limit is 5.
- The base turn draw is 2, adjusted by available hand space.
- Shields are attached to players, not teams.
- Team HP cannot exceed that match's initial HP.
- Each Player owns one Formation Area, which contains at most one Formation.
- A Covered Passive is a face-down Formation in its owner's Formation Area,
  not a separate Card container.
- The Game has at most one non-empty Turn Draw Pool.
- Cards in a Formation Area or the Turn Draw Pool are not in a hand or Discard
  Pile.
- Covered Passive Cards remain canonical hidden information in domain state and canonical events.
- Public counter effects are stored separately from hidden covered-passive cards.

### 2.1 Status Effects

Status duration uses explicit expiry timing:

```rust
enum StatusDuration {
    UntilTurnStart { player: PlayerId },
    UntilTurnEnd { player: PlayerId },
    Permanent,
}
```

Avoid generic `remaining_turns`, `remaining_rounds`, and `untilNextAction` in the core model. They are ambiguous in multiplayer and team mode. Rule resolvers translate rule text into explicit expiry timing.

`CannotAct` is the implementation spelling of the canonical status kind **Cannot Act**.

### 2.2 Game Conclusion

When a fully resolved semantic step produces a terminal outcome, record one
independent `GameConclusion` containing the `GameOutcome` and its explicit
`GameEndCause` or causes, then emit `GameEnded { conclusion }` as the last
canonical event. Neither the current Formation Area nor
`last_formation_by_player` is the authoritative source of the end reason.

Complete the current simultaneous semantic resolution before evaluating and
recording its conclusion. After `GameEnded`, stop: do not process later effects,
discard a remaining Formation merely for cleanup, enter `TurnDraw`, or enter
`TurnEnd`. Cards remain in the last Card zones established before the terminal
event, as required by official rule 7-3's immediate end and unfinished-effect
stop.

A public projection may render a source-Formation snapshot stored in the Game
Conclusion using Formation Area styling. That presentation does not make the
Formation Area itself the canonical end-reason record.

## 3) Commands

All gameplay changes come from validated commands and canonical automatic advancement.

Required command types:

- `PerformFormation`
- `PassAction`
- `ChooseTurnDiscard`
- `AnswerEffectChoice`
- `RetrievePreviousTurnDiscard` when Discard Retrieval is enabled

Optional future command types:

- active-effect commands that are not formation actions and do not close the player's action opportunity
- future action commands introduced by a ruleset when the action is genuinely not a formation use

Do not add:

- `EndActiveEffects`
- a standalone `ChangeClass` command for Metamorphosis (`幻化`)

Validation rules:

- reject if the command player is not the current player, unless a future rule explicitly allows out-of-turn input
- reject commands outside their valid phase
- reject illegal formation declarations, missing cards, duplicate submitted cards, and illegal declared targets
- validation failures emit no events and do not mutate state

The online adapter may store a pending command draft only while an
`EffectGenerated` choice suspends and later continues the Action that owns it.
A Turn Draw discard choice belongs to `TurnDraw` and must not retain the
preceding Action Command as a draft.

`PassAction` is legal only when the player has no cards in hand or has **Cannot
Act** status. A successful pass enters `Action`, processes the previous
Player's Covered Passive, and then advances to `TurnDraw` unless the game ends.

`RetrievePreviousTurnDiscard` is an active-effect command. It does not accept a
card target and does not consume the action opportunity; the rules derive the
sole Retrievable Discard from canonical turn history.

## 4) Formation Resolution

### 4.1 Shared Preliminaries

A formation definition identifies:

- formation id
- name
- formation category (`Attack` or `Spell`)
- formation pattern
- effect id

Formation matching decides only whether submitted card instances form the declared formation. Effect resolution is dispatched through the effect definition and effect plan.

If the same card instances can match multiple formations, the player or upper layer must explicitly declare `formation_id`. The engine does not infer or auto-select a formation.

Complete every `PerformFormation` validation before commitment. A failed
validation emits no event and changes no Card zone. After validation succeeds,
`FormationCommitted` atomically moves the submitted physical Cards from the
performing Player's hand to that Player's Formation Area, sets their initial
face state, and enters `Action`; later prevention or ineffectiveness never
returns them to hand.

Attack and immediate-Spell Formations commit face-up. A covered Passive Spell
commits face-down. Because Formation Areas belong to Players, the previous
Player's Covered Passive and the current Player's incoming Formation can occupy
their respective Formation Areas at the same time.

### 4.2 Attack Resolution

On `PerformFormation` whose effect plan is attack:

1. Validate current player, phase, formation declaration, submitted card instances, and declared targets.
2. Commit the Formation face-up to the performing Player's Formation Area.
3. Compute base points from the attack plan's point formula.
4. Flip, resolve, and discard the previous Player's Covered Passive, if present.
5. Resolve the attack target from current state. Base attack damage targets the previous player.
6. Collect every Pending Choice needed to determine the Attack Resolution
   without applying any part of that resolution early. An effect with an
   explicitly specified different timing remains outside this atomic result
   and is processed at its specified point.
7. Emit one atomic `AttackResolved` containing the damage resolution and every
   attached effect without another specified timing. Its replayable facts
   include the point breakdown, every HP and Shield delta, and all other effect
   deltas; none has an event-log order relative to another.
8. Apply the complete `AttackResolved` before evaluating Game Outcome. If
   `GameEnded` is emitted, stop. Otherwise emit `FormationCardsDiscarded`,
   moving the current Formation from its Formation Area to the applicable
   origin Discard Pile or Piles, and enter `TurnDraw`.

Five-element interaction:

- compare against the Five-Element Attack performed by the Previous Player during the immediately completed Previous Turn (rule 5-2.4b)
- current attack generates that previous-turn attack's element: heal target team (rule 5-2.4c)
- current attack overcomes that previous-turn attack's element: double damage (rule 5-2.4d)
- same element: halve damage and round up (rule 5-2.4e)
- unrelated element: normal damage
- if the target player has shield, skip five-element interaction entirely (rule 5-2.4b)

Physical attacks deal double damage to a player shield. Shield damage does not
pierce through to team HP.

If the Previous Player did not perform a Five-Element Attack during the
immediately completed Previous Turn, there is no Five-Element interaction to
resolve. Older attacks are outside the condition in rule 5-2.4b.

Water-Dotting Fan applies its Cannot Act and Cannot Draw effect only when the
Previous Player performed a Formation with at least four Cards during the
immediately completed Previous Turn. Profession Change is not a Formation and
does not satisfy this condition (Jianghu rule 4-7).

### 4.3 Immediate Spell Resolution

On `PerformFormation` whose effect plan is immediate spell:

- validate the formation use
- commit the Formation face-up to the performing Player's Formation Area
- flip and resolve the previous player's covered passive, if present
- resolve spell intents into semantic events
- request a Pending Choice when the Spell needs Player input; it suspends this
  `Action` without creating another phase
- after all effects and choices resolve, stop on `GameEnded`; otherwise emit
  `FormationCardsDiscarded` and enter `TurnDraw`

Metamorphosis keeps `metamorphosis` as the performed formation identity while
storing the copied category and effect separately. It keeps its active Spell Type,
recomputes point formulas from its own two Earth cards, and may establish a copied
counter effect publicly without covered cards.

Metamorphosis copies the category and rules text of the Formation performed by
the Previous Player during the immediately completed Previous Turn (basic rule
2-2.2). If that Player performed no Formation or performed a non-basic Formation,
there is no copy target (basic rule 2-2.4).

Radiance records the next player's hand as a canonical snapshot. Public event
filtering exposes the cards only to the formation player.

### 4.4 Covered Passive Resolution

On `PerformFormation` whose effect plan is covered passive:

- validate the formation use
- commit the Formation face-down to the performing Player's Formation Area
- flip and resolve the previous player's covered passive, if present
- mark the incoming Formation as sealed if an applicable seal modifies the
  cover action
- emit `PassiveCovered`, leave the Formation face-down in the Formation Area,
  and enter `TurnDraw` unless `GameEnded` has been emitted

At the next player's action start, the previous player's covered passive flips and attempts to affect that incoming action. The passive is discarded whether it applies or not.

The same flip-and-discard timing applies when the next player passes because no
action can be performed.

Rules:

- `Defense` applies only to incoming attacks.
- `Defense` prevents attack damage but not other attack effects such as Five Streams Unite's draw bonus.
- `Seal` applies only to incoming spells.
- If `Seal` applies to an incoming covered passive, the incoming passive remains covered and is marked sealed; it later flips as no effect.
- A Player's Formation Area can contain at most one Formation.

### 4.5 Other Actions

Profession Change is an Action Command, not a Formation Use. After validating
the declared Profession, prerequisite Profession, and selected Cards, it enters
`Action`, processes the previous Player's Covered Passive, and emits one
`ProfessionChanged` event that atomically changes Profession and moves the
selected Cards directly from hand to their applicable origin Discard Pile or
Piles. There is no temporary Card zone for those Cards.

`PassAction` enters `Action` without committing a Formation, processes the
previous Player's Covered Passive, and enters `TurnDraw` unless `GameEnded` has
been emitted.

## 5) Rulesets

A Ruleset is the mandatory Base Ruleset plus zero or more enabled Rule Modules.
All modules share one deterministic interface for:

- setup validation
- turn constants
- formation registry
- formation matching
- effect resolution
- event decisions

The Base Ruleset provides the 25 base formations and supports both two-player
and team-mode setup shapes. Rule Modules retain their official category:

- Advanced Rule Modules, such as Star
- Theme Rule Modules, such as Spirit
- Optional Rule Modules, such as Discard Retrieval and Personal Deck

The category affects presets, documentation, and UI, not the execution model.

Team mode is not a separate ruleset unless future rule behavior diverges. It is a setup shape with:

- exactly two teams
- the same number of players on each team
- every player assigned to a team
- every player appearing exactly once in turn order
- no adjacent players from the same team, including circular adjacency between the last and first player

### 5.1 Discard Retrieval

Discard Retrieval is independently configurable and resolves during
`ActiveEffects` as
an active effect.[3]

1. The Previous Player must have a Turn Draw Discarded Card from the immediately
   completed Previous Turn, and that Card Instance must still be in its Discard
   Pile.
2. The engine derives that card; the command does not submit a card ID.
3. The current Player's Team loses HP equal to the card's level times two.
4. The card moves to the top of the current Player's Deck. In a shared-deck game,
   this is the shared Deck.
5. HP change, card movement, and public exposure are one semantic resolution.

Discard Retrieval is optional, does not enter `Action`, and remains legal under
**Cannot Act**. Insufficient HP does not prevent it: HP falls to zero, retrieval
still resolves, and the game then ends.

### 5.2 Personal Deck

When Personal Deck is enabled:[3]

- each Player uses their own Deck and Discard Pile
- initial hands and later draws come from that Player's Deck
- each Deck List contains exactly 60 cards selected from one complete 90-card
  set
- each exact element/level combination permits at most four copies at levels
  1–3 and at most three copies at levels 4–5
- total card levels must not exceed 170

The built-in Preconstructed Deck List uses the following count for every
element:

| Level | Copies |
|---|---:|
| 1 | 3 |
| 2 | 2 |
| 3 | 3 |
| 4 | 2 |
| 5 | 2 |

This produces 60 cards with total level 170.

A valid custom Deck List is used when present. A missing or invalid custom list
automatically falls back to the Preconstructed Deck List, and the Player is
shown the name actually used.

Card Origin remains immutable. When Discard Retrieval or another effect such as
Chaos puts a card into a different Player's Deck:

- the destination is the performing Player's Deck
- the card remains fully public while in that Deck or hand
- using or discarding it sends it to its origin Player's Discard Pile

Every Player's Deck count and Discard Pile contents are public. Deck order,
ordinary opposing hands, Locked Deck List name, and Locked Deck List contents
remain private.

A **Discard Shuffle (洗棄牌)** occurs when an applicable Deck contains too few
Cards for a required draw, inspection, or search: the complete corresponding
Discard Pile is shuffled and placed at the bottom of that same Deck before the
operation continues. The Deck need not be empty; having fewer Cards than the
operation requires is sufficient. This is distinct from a **Deck Shuffle**,
which reorders Cards already in a Deck.

Fair Wind's Tailwind (`順風`) recovers only when a Discard Shuffle completes,
never from a Deck Shuffle. A shared-Deck Discard Shuffle recovers Tailwind for
every Player who currently owns that ability and whose Profession Abilities are
effective. With Personal Deck enabled, it recovers Tailwind only for the owner
of the shuffled Discard Pile, under the same conditions. A Player whose
Tailwind is already at its maximum has no recovery change.

Initial preparation shuffles, Rusted Iron Withered Forest, Sheep Stealing's
post-exchange shuffle, and 商調‧鳴金's post-search shuffle are Deck Shuffles and
therefore do not recover Tailwind. Any Discard Shuffle that those flows require
before drawing, inspecting, or searching remains a Tailwind recovery trigger.

### 5.3 Star

When the Star Rule Module is enabled:[5]

- a use of 鍠金、樸木、洄水、熾火、or 坱土 with at least 30 Attack Points
  summons its associated Star before elemental interaction, Environment Effects,
  Shields, or HP resolution can change the attack result
- a Team owns at most one Star, a newly summoned Star replaces its old Star,
  and one Star kind may be owned by only one Team
- summoning a Star also breaks its official opposing Star if currently owned
- each Team-owned Star enables its one-card fixed-10 Star strike, its three-card
  level-sum-times-three Star Formation, and one generating-element Card
  interpretation while matching a Base Ruleset Formation
- a three-card Star Formation grants one Turn Draw bonus and breaks its enabling
  Star after the attack, even when damage is prevented

虛空破星術 is an Active Spell made from three same-level Cards. Unless
cancelled by Seal, it breaks every owned Star and directly removes 20 HP from
each affected Team. All affected Teams resolve before Game Outcome evaluation.

Star Summoning history is retained per Player after Stars are replaced or
broken. A Player who personally summons all five Star kinds achieves Five-Star
Alignment; after the complete Formation Use resolves, their Team wins before
HP-based outcome evaluation.

### 5.4 Hero Schools

Hero Schools is one independently configurable Advanced Rule Module containing
all 18 Professions, their abilities and Formations, Profession Change, and Void
Reversion Technique.[4] This implementation target is the official 5.16 rules
retrieved on 2026-07-01; later changes to the mutable official `latest` pages
require a separate rule-upgrade decision.

- Every Player starts without a Profession and may own at most one.
- Profession Change is an Action Command, not a Formation Use. It validates the
  declared Profession, prerequisite Profession, and selected Cards, then uses
  the shared action-start and Counter Effect pipeline.
- The five Schools inherit lower-rank abilities through their Profession
  progression. Immortal and Saint do not retain the previous Profession's
  abilities.
- Automatic Profession Abilities and Formation Proficiencies apply
  automatically. Activated Profession Abilities resolve during `ActiveEffects`,
  do not enter `Action`, and share one successful activation allowance
  per Player turn.
- Formation Proficiencies add Player-scoped alternative matchers to the
  original Formation rather than creating new Formation identities.
- When one Card selection has multiple result-changing interpretations, the
  Player explicitly chooses a Formation Match Option.
- Prepared Profession Abilities such as Blazing Yang Art and Dark Spirit add
  turn-scoped interpretation or use restrictions to one physical Card Instance
  without mutating its Card Definition. Formation matching, point calculation,
  and effect resolution use the same effective element and level after all
  applicable Card Interpretation Layers. The ability and interpreted facts are
  public, while the physical target remains visible only to its owner until
  ordinary Card movement makes it public.
- Illusion and Phantasm discard two physical Cards and create one fixed Virtual
  Formation Card of a declared element and level. They do not target or
  reinterpret a third Card Instance. The virtual component is public when
  created, belongs to no Card zone, and cannot receive Fire Spirit, Pouch, or
  Star interpretations.
- Illusion requires that virtual component to perform the corresponding
  five-element strike during the same turn. Phantasm instead permits it to join
  any Base Ruleset Formation; it is a different Activated Profession Ability
  and does not trigger Illusion Refinement. A legal Formation Use records the
  physical and virtual components separately and fulfills the requirement even
  when the Formation is later ineffective or cancelled.
- Dark Spirit can only lower its selected physical Card to level one or two and
  requires that Card to join a Formation during the same turn. Blazing Yang Art
  raises its selected Wood or Fire Card by two, capped at five; it does not
  require using the Card, but that Card can participate only in a Base Ruleset
  Formation and cannot be used for Profession Change during the turn.
- Sacred Art may let one physical Card fill two Formation match slots, while
  Card movement and ordinary level-sum formulas count that Card only once.

Void Reversion Technique is an Active Spell made from three same-level Cards.
It changes the performing Player's Team HP by -20 and breaks every Profession;
when made from level-one or level-two Cards, Legendary Professions are retained.
Its HP delta, Profession Breaking, and Card movement resolve atomically before
Game Outcome evaluation.

### 5.5 Spirit

Spirit is a Theme Rule Module from the official 5.16 PDF, pages 25-26. It may be
enabled only with all three Advanced Rule Modules: Star, Five Directions
Legend, and Hero Schools. Optional Rule Modules remain independently
composable.

Each Player may own at most one Spirit. Different Players may own the same
Spirit kind. Spirit kind and current Spirit Power are public; a new Spirit
Summoning replaces the Player's old Spirit and starts the new Spirit at two
power.

Spirit Power ranges from zero through six:

- a Spirit breaks immediately when its power reaches zero
- a Player cannot dismiss their Spirit voluntarily
- choosing a same-element Turn Draw discard adds one power to that Player's
  Spirit, capped at six
- another Player's discard never charges the Spirit

The five Spirit-summoning Formations are Active Spells made from two
same-element Cards: 金靈喚術、木靈喚術、水靈喚術、火靈喚術、and 土靈喚術.

Spirit Skills resolve during `ActiveEffects`, do not enter `Action`, and remain
legal while the Player has **Cannot Act**. Each
individual Spirit may use one Skill per Player turn; replacing a Spirit creates
a new Spirit with its own allowance. A Skill validates only its printed timing,
inputs, and power cost. It may resolve with no benefit or a detrimental result,
and paid power is not refunded.

| Spirit | Skill | Cost | Effect |
|---|---|---:|---|
| Metal | 飛刃 | 2 | Previous Player's Team HP -10 |
| Metal | 劍雨 | 6 | Previous Player's Team HP -40 |
| Wood | 芬芳 | 2 | own Team HP +10 |
| Wood | 綻放 | 6 | own Team HP +40; also triggers automatically at zero HP |
| Water | 川流 | 1 | discard one selected hand Card; Turn Draw bonus +1 |
| Water | 浩瀚 | 3 | Turn Draw bonus +1 |
| Fire | 螢光 | 1 | one selected hand Card is level 3 for this turn |
| Fire | 絢爛 | 3 | one selected hand Card is a declared level 1-5 for this turn |
| Earth | 石盾 | 2 | the next Player's attack damage is ineffective during that Player's next turn |
| Earth | 岩壁 | 6 | construct a 40-point Shield |

Direct HP changes from Spirit Skills are not Attacks and bypass Shields and
Attack modifiers. Stone Shield prevents only attack damage; other effects of
the Attack still resolve, and the protection expires at the end of the next
Player's next turn even if they do not Attack. Because it is a Spirit Skill
rather than a Formation effect, it also prevents Sacred Beast attack damage.

Fire Skills add a turn-scoped Card Interpretation Layer without mutating the
Card Instance or Card Definition. Layers compose by dimension in effect order,
and a later layer replaces only the dimensions it specifies. Sacred Art
Multiplicity applies afterward: it may compose with a Fire level
interpretation, but it cannot use Star Element Substitution. Both Sacred Art
match slots retain the same element and level and cannot be reinterpreted
independently.

The canonical Fire Skill event records the selected Card Instance. Public views
show the Skill and declared level but reveal the selected Card only to its owner
until ordinary Card movement makes it public. Spirit Skill uses remain visible
in the Public Event Feed; Public State does not duplicate them as an
`already used` presentation field.

Automatic Bloom resolves before HP-based Game Outcome evaluation. Every
eligible six-power Wood Spirit owned by the defeated Team triggers
simultaneously; one canonical resolution consumes all triggering power and
recovers 40 HP per trigger, capped at initial Team HP. Direct victory such as
Five-Star Alignment does not trigger Bloom.

虛空碎靈術 is an Active Spell made from three same-level Cards. It atomically
reduces every Spirit's power by two before directly removing 20 HP for each
Spirit owner. All Spirit and Team deltas resolve before Game Outcome evaluation.
Bloom observes the reduced power, so a Wood Spirit reduced from six to four
cannot answer HP loss caused by that same resolution.

### 5.6 Pouch

Pouch is a Theme Rule Module from the
[official 5.16 complete rulebook][6], pages 37-38. This product intentionally
permits it only when Personal Deck and Spirit are enabled; Spirit transitively
requires all three Advanced Rule Modules.

Before initial hands are dealt, Players choose starting Pouches in Turn Order
from their unshuffled Personal Decks. Each choice removes one Card Instance from
that Deck and places it face down under its Pouch Owner. After all choices, a
trusted adapter shuffles the remaining Personal Decks and the engine records
those orders before dealing. Pouch identity is visible only to its owner until
the Card is revealed, while canonical events retain enough information for
replay.

This is a public, reconnectable Game Preparation lifecycle:
`InitialPouchSelection -> PendingDeckShuffle -> InitialDeal -> Ongoing`. It does
not add values to the Turn Phase model, and the first Turn begins only after the
initial deal completes.

Pouch knowledge is viewer-specific and persists across reconnects. An initial
Pouch is known only to its Pouch Owner. When Chain gives a Pouch to a teammate,
the performing Player retains knowledge because they selected the Card, and the
new Pouch Owner may also inspect it; every other viewer sees only a Card Back.
Once revealed or moved into a public Discard Pile, its identity is public.

Triggering a Pouch is an active effect during `ActiveEffects`, before the
Player's Action, and remains legal under **Cannot Act**. The Player reveals the
Card, chooses
exactly one Secret Strategy whose condition matches its printed element or
level, resolves that effect, and only then moves the revealed Card to its origin
Discard Pile.

Before submission, the Player may close the Pouch UI without changing canonical
state. Submission validates the selected Secret Strategy and all immediately
required input before emitting the reveal. Once revealed, the trigger cannot be
cancelled; any remaining target, Card, or branch choices are completed through
typed Pending Choices and replayable continuations. A validation failure emits
no reveal and leaves the Pouch in place.

A Secret Strategy that was legally triggered still reveals and discards its
source Card when prevention or current state makes its effect ineffective. For
example, Dark Crossing under an effect that forbids Profession Change performs
no Profession Change, and Retreat performs no Environment Clearing when no
Environment exists. An option that the published rule explicitly forbids is not
offered, such as Retreat's discard-and-transfer option when the Player has no
Card in hand.

The ten Secret Strategies use printed Pouch values:

| Strategy | Condition | Resolution |
|---|---|---|
| 金蟬 | Metal | Give the triggering Player Player-Only Secret Protection from Cannot Act, Cannot Draw, Counter Effects, and other Players' Secret Strategy effects for this Turn |
| 偷梁 | Wood | Cards in the triggering hand snapshot have level +1 until Turn End |
| 混水 | Water | Turn Draw bonus +1 this Turn |
| 觀火 | Fire | Protect the next Player's next Turn from attack damage and Formation-caused Team HP changes |
| 離山 | Earth | Suppress one Player's Profession Abilities and Spirit Skills and prevent Spirit Power gain for one Turn |
| 還魂 | Level 1 | Summon the source element's Spirit at one power plus the replaced Spirit's power, capped at six |
| 牽羊 | Level 2 | Exchange two selected Deck Cards for two selected Discard Pile Cards, then shuffle |
| 暗渡 | Level 3 | Directly change to the source element's first-tier Hero School Profession |
| 瞞天 | Level 4 | Break one selected existing Star or gain one selected Temporary Star Effect this Turn |
| 走為 | Level 5 | Clear the current Environment or discard one hand Card and transfer to its printed element's Environment |

Chain is an Active Spell made from three Cards with pairwise-different elements
and levels. It searches the performing Player's Personal Deck for exactly one
or two Cards:

- one selected Card becomes a Pouch owned by a selected friendly Player
- when a second Card is selected, it must differ in both element and level from
  the first and publicly triggers one eligible Secret Strategy immediately
- selecting zero Cards is not legal

When two Cards are selected, Chain first places the new Pouch. Any Pouch
previously owned by the target is immediately moved to its origin Discard Pile.
Chain then reveals the second Card and resolves its selected Secret Strategy,
setting that revealed source aside from the Personal Deck during resolution and
moving it to its origin Discard Pile only after the strategy finishes. It is
therefore not part of a Deck search or shuffle caused by that strategy. A
replaced Pouch already present in the performing Player's Discard Pile may be
selected by Sheep Stealing.

Pouch ownership does not change Card Origin. A Pouch given to a teammate by
Chain enters the origin Player's Discard Pile when it is triggered or replaced.
Chain does not shuffle after searching. Its private choice options expose
eligible Card Instances without revealing their Deck positions, and selected
Cards are removed while the relative order of the remaining Deck is preserved.
If the Deck contains fewer than two Cards when Chain begins, a Discard Shuffle
runs before the search as required by the published rule; the Player makes the
Chain selection only after that shuffle has completed.

Sheep Stealing first discards two selected Cards from the Player's Personal
Deck, then returns two selected Cards from that Player's Discard Pile and
shuffles. Official clarification 3-2.4 makes this ordering consequential: the
two Cards discarded by the first step are already in the Discard Pile during
the return selection, so either or both may be selected and returned
immediately. If the Deck contains fewer than two Cards when resolution begins,
its existing Discard Pile is Discard Shuffled first, and the Player selects the
exchange Cards only after that shuffle has completed. This preliminary Discard
Shuffle may recover Tailwind; Sheep Stealing's later post-exchange Deck Shuffle
does not. The Secret Strategy source Card enters the Discard Pile only after the
complete effect resolves and is therefore never one of the returned Cards.[6]

Dark Crossing causes a Direct Profession Change based on the source Card's
printed element: Metal to Warrior, Wood to Seeker, Water to Mesmer, Fire to
Mage, and Earth to Windwalker. It does not pay ordinary Profession Change Cards,
check prerequisite Professions, consume the Action opportunity, or enter the
ordinary Counter Effect pipeline. Effects that forbid Profession Change still
prevent it.

Deceive Heaven may grant the triggering Player one specified Temporary Star
Effect until Turn End. The grant includes that Star's element substitution and
both Star Formations, and it composes with a different Star already owned by the
Player's Team. It does not create or summon a Star and does not add Five-Star
Alignment history. When a granted Star Formation says to break its enabling
Star, it can break only the same-named Star owned by the performing Player's
Team; it never breaks an opposing Team's Star. If that Team does not own the
same-named Star, the breaking has no effect and the Temporary Star Effect still
lasts until Turn End. Deceive Heaven's alternative direct-breaking option is
different: it may target any existing Star, including one owned by either Team.

Lure the Tiger Away makes the selected Player's Profession Abilities and Spirit
Skills ineffective for its duration rather than prohibiting their use.
Activated Abilities and Skills may still be used, pay their costs, and consume
their usage allowances, but their effects do not execute. Automatic,
proficiency, and persistent abilities are suppressed. The affected Spirit also
cannot gain Spirit Power during that duration, including after replacement.
Return Soul may still replace or summon that Spirit at one initial power, but
the old Spirit's additional power cannot be inherited. Golden Cicada can protect
the Player-facing part of Lure the Tiger Away, but it does not restore Spirit
Skills or Spirit Power gain.

Steal the Beam snapshots the Card Instances in the triggering Player's hand and
gives those Cards level +1 until Turn End. Cards that enter the hand after the
strategy triggers are not affected. The interpretation remains attached to each
snapshotted Card Instance for the duration even if it temporarily leaves and
re-enters the hand.

Golden Cicada does not prevent an eligible Covered Passive from reaching its
ordinary trigger. The passive still flips, becomes public, and moves to its
origin Discard Pile; only its Counter Effect is ineffective against the
protected Player's action during that Turn.
Its protection applies only to the Player, not their Spirit, Team, Team Star, or
the shared Environment.

Watch the Fire prevents all attack damage during the next Player's next Turn,
including damage that a Shield would otherwise absorb. It also prevents Team HP
loss or recovery caused by Formations during that Turn. Other Formation effects,
Card movement, non-damage Shield reduction, and performance costs still resolve.
Effects from Spirit Skills, Secret Strategies, and other non-Formation sources
are outside this protection.

### 5.7 Tribulation

Tribulation is a Theme Rule Module from the official 5.16 PDF, page 36. It
requires Star, Five Directions Legend, and Hero Schools; Optional Rule Modules
remain independently composable.

The five Tribulations are variable-card-count Special Attacks. Each uses exactly
its named pair of overcoming elements, with one or more Cards of each element
and an effective level sum of at least seven for each element. They never
satisfy a rule that requires a fixed Formation card count.

| Formation | Elements | Attack | Additional effect |
|---|---|---:|---|
| 天雷劫火 | Metal and Fire | 60 | both Teams lose 15 HP |
| 烈風暴雨 | Fire and Water | 60 | every Player gains Gale-Rain Status for two turns |
| 泥石轟流 | Water and Earth | 60 or 80 | every Shield loses 20 points |
| 裂地崩山 | Earth and Wood | 60 | transfer the Environment and make every Player discard an Environment-Element Card or reveal their hand |
| 鏽鐵枯林 | Wood and Metal | 60 | reveal the top eight Cards of each applicable Deck, discard Cards of printed level three or higher, and shuffle the rest back |

Mudslide Torrent is 80 Attack Points only when its global Shield effect
effectively deducts at least one Shield point. Attack damage to a Shield does
not trigger the increase. Determine whether the target still has a Shield after
the global deduction: a remaining Shield receives the Attack; when that effect
breaks the target's Shield, the Attack reaches the Player.

Earth-Rending Mountain Collapse may select any Environment, including the
current one. Discard eligibility uses printed Card elements and the selected
Environment. Starting with the performing Player's Next Player, Players choose
sequentially; the performing Player is last. The declared Environment and
answers remain in the pending Formation command until every answer is present.
Under official rule 5-2.4i, Environment Transfer, discards or hand reveals, Attack
damage, and the rest of the Formation resolve atomically before Game Outcome
evaluation.

Rusted Iron Withered Forest processes a shared Deck once. With Personal Deck
enabled, it processes each Player-owned Deck separately. In either mode it
reorders Cards already in each applicable Deck, so it is a Deck Shuffle and
never recovers Tailwind.

Gale-Rain Status makes life recovery from Formations performed by its owner
ineffective; it does not block non-Formation recovery or Formations performed by
an unaffected teammate. Official rule 6-1 tracks each two-turn application
independently. The performing Player's current Turn End counts as their first
turn under the Status.

神算 is an Active Spell formed from one Card of effective level four or higher.
It removes every existing Divine Calculation Status, then grants its performing
Player one exclusive, non-stacking Divine Calculation Status. The Status lasts
until another 神算 removes it or the next Tribulation consumes it, including a
Tribulation performed by its owner.

When consumed, Divine Calculation makes that Tribulation's additional effect
ineffective for its owner. It also reduces Tribulation Attack damage received by
its owner by 20, including damage reflected to an attacking owner by
Countershock. A Shield takes unreduced damage because its owner did not
personally receive that damage.

The reduction is a rule of Divine Calculation Status itself, not a repeated
effect of the 神算 Formation. Snow-Treading Status therefore does not make a
Tribulation Attack bypass this reduction.

The additional-effect immunity applies by affected resource:

- Thunder-Fire Tribulation does not deduct 15 HP from the Status owner's Team.
- Gale-Rain Status is not added to the protected Player.
- Mudslide Torrent does not deduct points from the protected Player's Shield.
- Earth-Rending Mountain Collapse still transfers the shared Environment, but
  the protected Player neither discards nor reveals their hand.
- Rusted Iron Withered Forest still processes a shared Deck. With Personal Deck
  enabled, it skips the protected Player's Deck.

### 5.8 Web setup

New official rooms enable every available Rule Module by default, including
Pouch and its interactive Game Preparation stage. The Base Ruleset cannot be
disabled. Existing rooms retain their stored module configuration when a new
module becomes available.

The room owner may change Rule Modules subject to declared dependencies.
Enabling a Rule Module automatically enables its transitive dependencies;
disabling a dependency automatically disables every dependent Rule Module.
Server-side setup validation rejects any invalid dependency combination.

The room-creation dialog intentionally omits Rule Module controls. In the
waiting room, Base is shown as fixed-on and each Rule Module has one owner-only
toggle grouped by its official category. The enabled module list is also visible
during an active match and in room-list metadata.

The Web application stores one named custom Deck List per account. A minimal
editor lives at `/deck`, linked from the account menu immediately above logout.
It uses an element-by-level grid and displays live card-count, total-level, and
copy-limit validation.

For non-owners, becoming ready captures the effective Locked Deck List. The room
owner's list is captured when starting the game. Changing any Rule Module,
cancelling readiness, or leaving invalidates affected waiting-room snapshots.
Started-game snapshots are immutable. Only the owning Player sees a Locked Deck
List's name, contents, or fallback notice.

When Pouch is enabled, starting the room still immediately locks membership,
Rule Modules, and Deck Lists and routes Players to the game. The room is already
started while `InitialPouchSelection` and `PendingDeckShuffle` are in progress;
these reconnectable steps do not run in the waiting room.

## 6) Persistence And Replay

The canonical game record is event-sourced:

```rust
struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    latest_snapshot: Option<GameSnapshot>,
}
```

The event log is the replay source of truth. Snapshots are optional cache/checkpoint data.

Opening events preserve the existing shared-deck shape and add an explicit
per-Player shape:

```rust
GameEvent::DeckPrepared {
    deck_order: Vec<CardInstanceId>,
}

GameEvent::PlayerDeckPrepared {
    player: PlayerId,
    deck_order: Vec<CardInstanceId>,
}
```

Replay uses recorded events directly and does not rerun RNG. A seed may be stored as metadata/debug context when generating the initial deck, but once `DeckPrepared` exists, deck order is authoritative.

Initial hands are dealt by the engine from the applicable prepared Deck order
and emitted as events:

- first player receives 4 cards
- all other players receive 5 cards

Canonical events may contain hidden information needed for replay. Public views and public event feeds filter hidden information per viewer and are not replay sources.

Canonical event and pending-choice payloads are persisted record formats. Adding or changing their fields requires an explicit record migration that preserves replay verification for existing rooms. Presentation-only metadata, such as the number of cards required by an effect choice, is derived by the Web projection instead of being added to canonical payloads.

## 7) Event Shape

Use semantic events with explicit replayable deltas.

Avoid events that are too vague to replay without recomputing rules:

```rust
GameEvent::FormationPerformed { formation_id: FormationId }
```

Do not add an abstract `FormationUseCompleted` event. Completion is represented
by the next rule-significant fact: `FormationCardsDiscarded` for a resolved
Attack or immediate Spell, `PassiveCovered` for a waiting Passive Spell, or
`GameEnded` for a terminal resolution.

Also avoid reducing the log to only low-level mutations with no domain meaning.

Prefer semantic events such as:

```rust
GameEvent::AttackResolved {
    attacker: PlayerId,
    target: PlayerId,
    formation_id: FormationId,
    resolution: AttackResolution,
    elemental_context_update: Option<LastElementalAttackUpdate>,
}

struct AttackResolution {
    outcome: ActionOutcome,
    point_breakdown: AttackPointBreakdown,
    damage: AttackDamageResolution,
    additional_effects: ResolvedAttackEffects,
}

GameEvent::TurnDrawResolved {
    player: PlayerId,
    discard: CardInstanceId,
    kept_cards: Vec<CardInstanceId>,
}

GameEvent::GameEnded {
    conclusion: GameConclusion,
}
```

Formation Card movement has its own semantic boundaries:

- `FormationCommitted` moves submitted physical Cards from hand to the
  performing Player's Formation Area after validation succeeds.
- `FormationCardsDiscarded` moves a resolved face-up Formation from its area to
  the applicable origin Discard Pile or Piles.
- `PassiveCovered` records that a committed passive Formation remains
  face-down and waiting in its owner's Formation Area.
- `TurnDrawResolved` records both the selected discard and every Card kept from
  the Turn Draw Pool. Replay applies the discard before moving kept Cards into
  hand, but no canonical event exists between those moves.
- `GameEnded` records the independent Game Conclusion and is always the last
  canonical event.

Official rules 2-5.3c and 5-2.4i make Attack damage and every attached effect
without a different specified timing one simultaneous Attack Resolution.
`AttackResolved` therefore stores all of those resolved consequences in one
event and applies them atomically. Do not emit separate `HpChanged`,
`ShieldChanged`, status, Card-movement, or bonus events for consequences inside
that same Attack Resolution; their sequence would invent a rule order that does
not exist. Effects with an explicit different timing retain their own semantic
event at that timing.

A single command may emit multiple events, for example:

- `FormationCommitted`
- `PassiveFlipped`
- `CounterEffectResolved`
- `FormationEffectCopied`
- `HandInspected`
- `AttackResolved`
- `EffectChoiceRequested`
- `FormationCardsDiscarded`

Do not persist a separate command log. Recorded-event metadata is enough to correlate accepted commands with emitted events.

## 8) Error Taxonomy

Use three error layers:

- `Validation`: submitted command or setup is illegal for the current rules and state
- `RuleImplementation`: known legal rule path is incomplete or internally inconsistent
- `EngineInvariant`: impossible state or core invariant violation

All errors are atomic:

- emit no events
- mutate no state
- consume no action opportunity

A known formation with legal cards but missing resolver is a rule implementation error, not a validation failure.

## 9) Testing Checklist

- Public phase flow stops only when player input is needed.
- `ActiveEffects` allows zero or more Active-Effect Commands before one Action
  Command enters `Action`.
- No `EndActiveEffects` command is required or event-logged.
- `Action` remains current across every effect-generated Pending Choice.
- `PassAction` is legal only for no hand or **Cannot Act**.
- A validated Formation moves from hand to its Player-owned Formation Area
  before effects begin and is never rolled back by prevention or
  ineffectiveness.
- Turn Draw moves N+1 Cards to the Turn Draw Pool, and only the selected discard
  and remaining kept Cards leave it during `TurnDrawResolved`.
- Drawn Cards are not part of the hand while the discard Pending Choice remains
  unresolved.
- The Turn Draw discard Pending Choice remains in `TurnDraw` and can be replayed.
- Discard Shuffles record shuffled order and do not rerun RNG on replay.
- Attack base damage targets previous player and resolves HP to that player's team.
- One atomic `AttackResolved` contains Attack damage and every attached effect
  without another specified timing; replay and Public Views never expose an
  invented order among those consequences.
- Two-player mode still uses team-owned HP.
- Five-element interaction uses only a Five-Element Attack performed by the Previous Player during the immediately completed Previous Turn and is disabled by target shield.
- Physical attacks deal double damage to shields.
- Covered passive flips at next player's action start and is discarded whether it applies or not.
- Covered passive also flips when that action is passed.
- Each Player owns one Formation Area containing at most one Formation; a
  Covered Passive is a face-down state in that area, not a separate zone.
- `Defense` applies only to attacks.
- `Seal` applies only to spells, and seals incoming covered passives without revealing them early.
- Metamorphosis preserves its name, stores the copied resolved effect, and copies counter effects without copying passive performance procedure.
- Recovery is capped at the match's initial HP.
- Radiance hand snapshots are visible only to the formation player.
- Public state views and public event feeds do not leak hidden hands, covered cards, draw choices, or effect-choice options.
- Canonical events remain complete enough for replay.
- `GameEnded` carries the Game Conclusion, is last, and prevents Formation
  cleanup, Turn Draw, Turn End, and all later unfinished processing.
- Team-mode setup validation rejects invalid seating and unequal teams.
- Every enabled Rule Module executes through the same Ruleset interface.
- Discard Retrieval derives the Previous Player's Previous Turn discard and
  emits no events when validation fails.
- Discard Retrieval remains legal under **Cannot Act** and can end the game by
  reducing the current Player's Team HP to zero.
- Personal Deck validation enforces 60 cards, official copy limits, and total
  level at most 170.
- The Preconstructed Deck List has per-element counts `3/2/3/2/2` and total
  level 170.
- Personal Deck draw, Discard Shuffles, and initial deal use the correct
  Player-owned piles.
- Exposed Foreign Cards remain public and return to their origin Discard Pile.
- Locked Deck Lists and ordinary opposing hands remain private.
- Rule Module changes invalidate waiting-room readiness and deck snapshots.
- Spirit setup requires Star, Five Directions Legend, and Hero Schools.
- Spirit Summoning replaces the old Player-owned Spirit at two power.
- Matching Turn Draw discards charge only the discarding Player's Spirit and
  never exceed six power.
- Spirit Skills remain active effects under **Cannot Act**, consume power even
  when ineffective, and allow one successful use per Spirit instance per turn.
- Public State exposes Spirit kind and power without exposing Fire's selected
  hidden Card or duplicating Skill-use history as presentation state.
- Fire Card Interpretation Layers compose by dimension and expire at Turn End.
- Sacred Art never combines with Star Element Substitution and never gives its
  two match slots independent interpretations.
- Stone Shield expires after the next Player's next turn, prevents only attack
  damage, and applies to Sacred Beast damage.
- Simultaneous Bloom and Void Spirit-Shattering replay without transient
  winners or event-order-dependent outcomes.
- Pouch requires Personal Deck and Spirit, with all three Advanced Rule Modules
  inherited through Spirit.
- Game Preparation records private initial Pouch choices, trusted per-Player
  Deck shuffles, and the initial deal before the first Turn.
- Every Secret Strategy and Chain branch has Rust conformance, replay, and
  Public View coverage.
- New multiword Pouch command and event fields retain exact camelCase Web DTO
  serialization.
- Brave Playwright covers initial Pouch selection, privacy, reconnect, and one
  Chain-to-teammate strategy flow without duplicating all ten strategies in the
  browser suite.

[1]: https://www.cfecards.org/rule/latest/you-xi-gui-ze '五行戰鬥牌官方網站 - 遊戲規則（完整規則書）'
[2]: https://www.cfecards.org/rule/latest/basicrule '五行戰鬥牌官方網站 - 基礎規則'
[3]: https://www.cfecards.org/rule/latest/xuan-yong-gui-ze-qi-pai-gui-ze-ge-ren-pai-zu '五行戰鬥牌官方網站 - 選用規則：棄牌回收、個人牌組'
[4]: https://www.cfecards.org/rule/latest/hero '五行戰鬥牌官方網站 - 進階規則‧英雄學派'
[5]: https://www.cfecards.org/rule/latest/star '五行戰鬥牌官方網站 - 進階規則‧星辰圖記'
[6]: https://www.dropbox.com/scl/fi/4cigz3t61p07l5tkgvjyl/5.16.pdf?dl=0&rlkey=m2zx1x5dwb6cw7llyy8wt9xiy&st=mqligrpj '五行戰鬥牌官方 5.16 完整規則書'
