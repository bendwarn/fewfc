> 規則依據：官方「遊戲規則（完整規則書）」中的回合流程、施展陣法步驟、目標定義、被動術式（蓋牌）觸發點、以及基礎規則陣法列表。([cfecards.org][1])

---

# CFECards Rules Engine Reference

This document is a rulebook-oriented reference for the Rust rules engine. Domain language is defined in [`../CONTEXT.md`](../CONTEXT.md), and implementation decisions are recorded in [`rules-engine-decisions.md`](./rules-engine-decisions.md).

When this document describes implementation shape, it follows those two documents.

## 0) Goal

Build a deterministic CFECards rules engine that:

- Enforces fixed turn flow with a public phase model: `TurnStart -> Main -> TurnDraw -> TurnDrawDiscardChoice? -> TurnEnd`.
- Supports `Main` as the public input phase where a player may use zero or more active-effect commands before exactly one action command closes the phase.
- Supports action commands such as `PerformFormation` and `PassAction`; future rulesets may add more action commands.
- Treats class change (`幻化`) as a formation use, not as a standalone `ChangeClass` command.
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

Public phases:

1. `TurnStart` - start-of-turn timing and status expiration.
2. `Main` - accepts active-effect commands, then exactly one action command.
3. `TurnDraw` - automatic draw pipeline.
4. `TurnDrawDiscardChoice` - only present when draw creates a pending discard choice.
5. `TurnEnd` - end-of-turn timing and status expiration.

Rulebook timing names such as `ActiveWindow` and `Action` remain useful concepts, but they are not separate public phases. The engine does not require or event-log an `EndActiveWindow` command. The first successful action command implicitly closes `Main`.

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

Do not model elemental attack, physical attack, special attack, active spell, passive spell, or class change as separate formation categories.

Execution details belong to effect plans:

- an attack effect plan may be elemental, physical, or special
- a spell effect plan may resolve immediately or be covered as a passive
- class change (`幻化`) is a basic formation handled through the normal `PerformFormation` pipeline

### 1.5 Card Instances And Card Definitions

Zones store card instances, not card definitions.

- `CardInstanceId` identifies one movable card in the match.
- `CardDefId` identifies immutable printed-card data.
- `CardDef` contains stable printed-card data such as name, element, and level.

Rules resolve `CardInstanceId -> CardDefId -> CardDef` when matching formations or computing effects.

## 2) State Model

Minimum shape:

```rust
enum Phase {
    TurnStart,
    Main,
    TurnDraw,
    TurnDrawDiscardChoice,
    TurnEnd,
}

struct GameSetup {
    ruleset: RulesetId,
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    hp: Vec<TeamHp>,
    card_defs: Vec<CardDef>,
    card_instances: Vec<CardInstanceDef>,
    hand_limit: usize,
    base_draw: usize,
}

struct GameState {
    status: GameStatus,
    turn_number: u64,
    phase: Phase,
    current_turn_index: usize,
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    hp: Vec<TeamHp>,
    initial_hp: Vec<TeamHp>,
    deck: Vec<CardInstanceId>,
    hands: Vec<PlayerHand>,
    discard: Vec<CardInstanceId>,
    pending_choice: Option<PendingChoice>,
    shields: Vec<PlayerShield>,
    covered_passives: Vec<CoveredPassive>,
    counter_effects: Vec<CounterEffect>,
    statuses: Vec<StatusEffect>,
    last_formation_by_player: HashMap<PlayerId, LastFormationUse>,
}
```

Notes:

- `deck[0]` is the top of the deck.
- The base hand limit is 5.
- The base turn draw is 2, adjusted by available hand space.
- Shields are attached to players, not teams.
- Team HP cannot exceed that match's initial HP.
- Covered passive cards remain canonical hidden information in domain state and canonical events.
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

## 3) Commands

All gameplay changes come from validated commands and canonical automatic advancement.

Required command types:

- `PerformFormation`
- `PassAction`
- `ChooseTurnDiscard`
- `AnswerEffectChoice`

Optional future command types:

- active-effect commands that are not formation actions and do not close the player's action opportunity
- future action commands introduced by a ruleset when the action is genuinely not a formation use

Do not add:

- `EndActiveWindow`
- a standalone `ChangeClass` command for base class change (`幻化`)

Validation rules:

- reject if the command player is not the current player, unless a future rule explicitly allows out-of-turn input
- reject commands outside their valid phase
- reject illegal formation declarations, missing cards, duplicate submitted cards, and illegal declared targets
- validation failures emit no events and do not mutate state

The online adapter may store a pending command draft only while an `EffectGenerated` choice suspends and later continues formation resolution. `TurnDrawDiscard` is normal turn completion and must not retain the preceding formation command as a draft.

`PassAction` is legal only when the player has no cards in hand or has **Cannot Act** status. A successful pass consumes the action opportunity and advances toward turn draw.

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

### 4.2 Attack Resolution

On `PerformFormation` whose effect plan is attack:

1. Validate current player, phase, formation declaration, submitted card instances, and declared targets.
2. Flip and resolve the previous player's covered passive, if present.
3. Resolve the attack target from current state. Base attack damage targets the previous player.
4. Compute base points from the attack plan's point formula.
5. Resolve shield absorption and five-element interaction.
6. Emit a semantic attack event with explicit replayable deltas, including point breakdown, HP delta, shield delta if any, and card move deltas.

Five-element interaction:

- current attack generates the previous player's immediately preceding elemental formation: heal target team
- current attack overcomes the previous player's immediately preceding elemental formation: double damage
- same element: halve damage and round up
- unrelated element: normal damage
- if the target player has shield, skip five-element interaction entirely

Physical attacks deal double damage to a player shield. Shield damage does not
pierce through to team HP.

Physical attacks, special attacks, and spells break the previous-turn elemental
context. Older elemental attacks do not remain eligible for interaction.

### 4.3 Immediate Spell Resolution

On `PerformFormation` whose effect plan is immediate spell:

- validate the formation use
- flip and resolve the previous player's covered passive, if present
- resolve spell intents into semantic events
- request a pending choice when a spell needs player input
- record formation-use card movement explicitly through card move deltas or equivalent replayable deltas

Class change keeps `metamorphosis` as the performed formation identity while
storing the copied category and effect separately. It keeps its active Spell Type,
recomputes point formulas from its own two Earth cards, and may establish a copied
counter effect publicly without covered cards.

Radiance records the next player's hand as a canonical snapshot. Public event
filtering exposes the cards only to the formation player.

### 4.4 Covered Passive Resolution

On `PerformFormation` whose effect plan is covered passive:

- validate the formation use
- flip and resolve the previous player's covered passive, if present
- place submitted card instances into the covered passive zone
- mark the covered passive as sealed if an applicable seal modifies the incoming cover action
- record all zone movement explicitly

At the next player's action start, the previous player's covered passive flips and attempts to affect that incoming action. The passive is discarded whether it applies or not.

The same flip-and-discard timing applies when the next player passes because no
action can be performed.

Rules:

- `Defense` applies only to incoming attacks.
- `Defense` prevents attack damage but not other attack effects such as Five Streams Unite's draw bonus.
- `Seal` applies only to incoming spells.
- If `Seal` applies to an incoming covered passive, the incoming passive remains covered and is marked sealed; it later flips as no effect.
- A player can have at most one pending covered passive.

## 5) Rulesets

A ruleset is a complete deterministic rule module for:

- setup validation
- turn constants
- formation registry
- formation matching
- effect resolution
- event decisions

The base ruleset provides the 25 base formations and supports both two-player and team-mode setup shapes.

Team mode is not a separate ruleset unless future rule behavior diverges. It is a setup shape with:

- exactly two teams
- the same number of players on each team
- every player assigned to a team
- every player appearing exactly once in turn order
- no adjacent players from the same team, including circular adjacency between the last and first player

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

Opening events include the full prepared deck order:

```rust
GameEvent::DeckPrepared {
    deck_order: Vec<CardInstanceId>,
}
```

Replay uses recorded events directly and does not rerun RNG. A seed may be stored as metadata/debug context when generating the initial deck, but once `DeckPrepared` exists, deck order is authoritative.

Initial hands are dealt by the engine from the prepared deck order and emitted as events:

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

Also avoid reducing the log to only low-level mutations with no domain meaning.

Prefer semantic events such as:

```rust
GameEvent::AttackResolved {
    attacker: PlayerId,
    target: PlayerId,
    formation_id: FormationId,
    used_cards: Vec<CardInstanceId>,
    point_breakdown: AttackPointBreakdown,
    hp_change: HpChangeDelta,
    shield_change: Option<ShieldChangeDelta>,
    card_moves: Vec<CardMoveDelta>,
    elemental_context_update: Option<LastElementalAttackUpdate>,
}
```

A single command may emit multiple events, for example:

- `PassiveFlipped`
- `CounterEffectResolved`
- `FormationEffectCopied`
- `HandInspected`
- `AttackResolved`
- `EffectChoiceRequested`

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
- `Main` allows zero or more active-effect commands before one action command.
- No `EndActiveWindow` command is required or event-logged.
- `PerformFormation` and `PassAction` close `Main`.
- `PassAction` is legal only for no hand or **Cannot Act**.
- Turn draw uses "draw N+1, choose one newly drawn card to discard".
- Turn draw discard creates a pending choice and can be replayed.
- Discard recycling records shuffled order and does not rerun RNG on replay.
- Attack base damage targets previous player and resolves HP to that player's team.
- Two-player mode still uses team-owned HP.
- Five-element interaction uses only the previous player's immediately preceding formation and is disabled by target shield.
- Physical attacks deal double damage to shields.
- Covered passive flips at next player's action start and is discarded whether it applies or not.
- Covered passive also flips when that action is passed.
- `Defense` applies only to attacks.
- `Seal` applies only to spells, and seals incoming covered passives without revealing them early.
- Class change preserves its name, stores the copied resolved effect, and copies counter effects without copying passive performance procedure.
- Recovery is capped at the match's initial HP.
- Radiance hand snapshots are visible only to the formation player.
- Public state views and public event feeds do not leak hidden hands, covered cards, draw choices, or effect-choice options.
- Canonical events remain complete enough for replay.
- Team-mode setup validation rejects invalid seating and unequal teams.

[1]: https://www.cfecards.org/rule/latest/you-xi-gui-ze '五行戰鬥牌官方網站 - 遊戲規則（完整規則書）'
[2]: https://www.cfecards.org/rule/latest/basicrule '五行戰鬥牌官方網站 - 基礎規則'
