# CFECards Rules Engine Decisions

This document records implementation decisions clarified before the Rust implementation.

## Confirmed

### 1. Implementation Boundary

Build the rules engine as a pure Rust library crate.

The initial implementation is responsible for deterministic game rules only:

- validating commands
- advancing turn/phase state
- resolving formations
- emitting events
- supporting snapshot persistence
- supporting event-log replay

The initial implementation does not include UI, networking, or a CLI. Those can be added later as adapters around the library.

### 2. Architecture

Use Clean Architecture boundaries.

The domain core owns game concepts and rule invariants. Application services orchestrate commands and replay. Infrastructure code handles serialization, storage, RNG/deck initialization, and any future adapters.

The domain layer must not depend on UI, storage, network, or framework concerns.

### 3. Crate And Module Shape

Start with a single Rust library crate and enforce Clean Architecture through internal module boundaries.

Planned module shape:

```text
src/
  domain/          # GameState, PlayerId, CardId, Phase, Formation, rule invariants
  application/     # Command handling, turn orchestration, replay service
  rules/           # BasicRuleModule, TeamModeModule, formation definitions/effects
  ports/           # traits: EventStore, SnapshotStore, RngSource if needed
  infrastructure/  # serde persistence, deterministic deck/shuffle helpers
```

Do not split into a Rust workspace at the start. The module boundaries should make a future split into multiple crates straightforward if the project grows.

### 4. Persistence Source Of Truth

Use the event log as the canonical game record. Snapshots are optional cache/checkpoint data for faster loading.

Each accepted command emits one or more `GameEvent` values. `GameState` is a projection derived from initial setup plus replayed events.

Primary persisted shape:

```rust
struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    latest_snapshot: Option<GameSnapshot>,
}
```

Loading may use `latest_snapshot` as an optimization, but replay remains the authoritative behavior. Tests must verify that direct command execution and replay produce the same state.

### 5. Randomness And Deck Order

Persist the full prepared deck order in the event log. A seed may be stored as metadata/debug context, but replay must not depend on any RNG algorithm.

Opening event shape:

```rust
GameEvent::DeckPrepared {
    deck_order: Vec<CardInstanceId>,
}
```

Replay uses `deck_order` directly. Tests may provide fixed deck orders without invoking RNG.

If seed-based deck generation is added later, it happens only when creating the initial game record. Once `DeckPrepared` is emitted, the event log's deck order is authoritative.

### 6. Players, Teams, And HP Ownership

Use one target/HP model for both 2-player and team modes.

HP is owned by teams:

```rust
struct Player {
    id: PlayerId,
    team: TeamId,
}

struct GameState {
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    hp: HashMap<TeamId, i32>,
}
```

In 2-player mode, each player belongs to their own single-player team. In team mode, multiple players may belong to the same team.

Turn order is always player-based. Damage targets a player first, then resolves to that player's `TeamId` for HP changes.

### 7. Hidden Information And Covered Passives

The internal domain state stores complete hidden information. Public/player-specific views filter hidden information at the boundary.

Internal state example:

```rust
struct PlayerFormationArea {
    player: PlayerId,
    formation: Option<FormationInArea>,
}

enum FormationAreaState {
    FaceUpResolving,
    FaceDownResolving,
    FaceDownWaiting { sealed: bool },
}

enum PublicCardRef {
    Known(CardInstanceId),
    Hidden { count: usize },
}
```

Every Player owns one Formation Area and it contains at most one Formation. A
Covered Passive is the `FaceDownWaiting` state of a Formation in that area, not
a separate Card container or zone. An incoming Player may therefore have a
face-up Formation in their own area while the previous Player's Covered Passive
still occupies the previous Player's area.

`FormationCommitted` and `PassiveCovered` events store the real
`CardInstanceId` values so replay remains deterministic and complete.

External views are generated per viewer:

- the Formation Area owner can see their own covered cards
- other players only see hidden card counts

Hidden information filtering is an API/presentation concern. It must not weaken the domain state or replay model.

### 8. Passive Reveal Timing

Model covered passive spells with one canonical trigger timing:

```rust
enum PassiveTriggerTiming {
    NextPlayerActionStart,
}
```

When player A covers a passive on their action, it is checked at the start of the next player's action resolution. If player B is next in turn order, then A is B's previous player (`上家`), so the rulebook's "flip previous player's passive before attack/spell resolution" is this same timing.

Resolution order:

1. Validate B's complete Action Command. Validation failure emits no events and
   changes no state.
2. If the command is `PerformFormation`, emit `FormationCommitted` to move its
   physical Cards from B's hand to B's Formation Area. A successful Action
   Command enters `Action`; commitment is not rolled back later.
3. The engine checks A's Formation Area for a Covered Passive.
4. If present, the passive flips and attempts to affect B's Action.
5. The passive moves from A's Formation Area to the applicable origin Discard
   Pile or Piles whether or not its effect applies.
6. B's Action continues unless the passive changes or prevents its result.

Do not implement separate passive trigger systems for attacks and active spells.

### 9. Validation Failure Versus Resolution Outcome

Validation failure is not a game event.

If a command fails validation, the engine returns an error and does not change state, consume the action, or append to the event log.

Validation failures include:

- wrong phase
- wrong player
- missing cards
- invalid formation
- illegal target declaration

Once validation succeeds and action resolution begins, the action is part of the game history and consumes the turn's action even if the final outcome is prevented or has no effect.

Resolution outcomes are recorded inside the semantic resolution that actually
occurred, for example an Attack Resolution:

```rust
struct AttackResolution {
    outcome: ActionOutcome,
    // point breakdown and resolved consequences
}

enum ActionOutcome {
    Applied,
    Prevented { reason: PreventionReason },
    NoEffect { grounds: Vec<NoEffectGround> },
}
```

An applicable effect produces exactly one outcome. `grounds` contains every
independently sufficient No-Effect Ground; it has a deterministic serialized
order only, never a primary cause or domain priority. Check applicability before
collecting grounds, so an inapplicable Defense facing a Spell records only
`NotAnAttack` rather than unrelated immunity grounds.

This preserves exactly-one-action turn semantics without treating invalid
commands as historical facts. Do not add an abstract `FormationUseCompleted`
event: normal Formation completion is the real `FormationCardsDiscarded` or
`PassiveCovered` fact, while terminal resolution ends with `GameEnded`.

### 10. Turn Draw Pending Choice

Source basis: official rule 4-2.4d draws one more Card than the number that will
be added to hand, has the Player choose one of those Cards to Discard, and only
then adds the remaining Cards to hand.

Implementation interpretation: the pre-discard Cards need a canonical zone
that is neither hand nor Discard Pile, and waiting for this choice is state
inside the official Turn Draw stage rather than a sixth Turn phase.

Turn Draw requires a Pending Choice because the Player draws `N + 1` Cards and
chooses one of those newly drawn Cards to discard. It remains entirely within
the canonical `TurnDraw` phase; waiting for the choice is state within that
phase, not another phase.

Use one game-scoped Turn Draw Pool:

```rust
struct GameState {
    turn_draw_pool: Vec<CardInstanceId>,
    pending_choice: Option<PendingChoice>,
}

struct PendingChoice {
    player: PlayerId,
    kind: PendingChoiceKind,
}

enum PendingChoiceKind {
    TurnDrawDiscard {
        allowed_discards: Vec<CardInstanceId>,
    },
}
```

Flow:

1. Enter `TurnDraw`.
2. Emit `CardsDrawnForTurnDiscardChoice` to move `N + 1` Cards from the
   applicable Deck into the Turn Draw Pool and create its discard Pending
   Choice. The Cards are not in the Player's hand.
3. Accept `ChooseTurnDiscard` while the phase remains `TurnDraw`.
4. Emit one atomic `TurnDrawResolved { discard, kept_cards }`. The selected Card
   enters its applicable origin Discard Pile first; the remaining Cards then
   enter the Player's hand. There is no Player input or canonical replay state
   between those movements.
5. Clear the Pool and enter `TurnEnd`.

This supports mid-draw save/load, UI waiting states, online play, and deterministic replay.

`TurnDrawResolved` replaces `TurnDiscardChosen` for the new canonical record
version. Implementing this decision requires the explicit persisted-record
migration already required for canonical event changes; snapshots are caches
and may be rebuilt from migrated events.

### 11. Draw Limits, Hand Limit, And Discard Shuffles

Hand limit is 5.

If the player's hand is already at the hand limit, skip turn draw and do not create a pending choice.

Otherwise, turn draw uses the rule intent "draw `N + 1`, then choose 1 newly drawn card to discard", where `N = min(base_draw, available_hand_space)`.

This means a Player with at least one available hand slot can have `N + 1` Cards
waiting in the Turn Draw Pool while the existing hand remains unchanged. The
Player never temporarily holds a Card above the hand limit.

`allowed_discards` must contain only the Cards in that Turn Draw Pool. Cards
that were already in the Player's hand before the draw are not legal choices
for `ChooseTurnDiscard`.

When the Deck is insufficient, perform a **Discard Shuffle (洗棄牌)** first:
shuffle the complete applicable Discard Pile and place every shuffled Card at
the bottom of that same Deck. Then continue drawing. A non-empty Deck still
requires a Discard Shuffle when it contains fewer Cards than the draw requires.

Deck direction:

- `deck[0]` is the top of the deck and the next card drawn.
- `deck.last()` is the bottom of the deck.

Discard Shuffles use the trusted randomness decision boundary. The canonical
request and result distinguish shuffling the complete Discard Pile into the
bottom of its Deck from reordering Cards already in a Deck, and replay consumes
the recorded order without rerunning RNG. The continuation describes what
resumes after the shuffle; it does not classify which kind of shuffle occurred.

Cards are eligible for a Discard Shuffle based on their current zone.

- Formation Cards that already moved to a Discard Pile before `TurnDraw` may
  participate in a Discard Shuffle during that same Turn Draw
- Cards in the Turn Draw Pool are not in a Discard Pile and cannot participate
  in that same Discard Shuffle
- the Card selected by `ChooseTurnDiscard` enters its applicable origin Discard
  Pile only when `TurnDrawResolved` applies

If the player has available hand space but the Deck plus its applicable Discard
Pile cannot satisfy the required draw count, report an engine error. Do not
perform a partial draw and do not create a pending choice.

### 12. Passing The Action Phase

Do not automatically skip the `Action` phase. Use an explicit command:

```rust
Command::PassAction {
    reason: PassActionReason,
}

enum PassActionReason {
    NoCardsInHand,
    CannotActByStatus,
}
```

Validation:

- if the player can perform a legal action, `PassAction` fails validation
- if the player has no cards in hand, `PassAction { reason: NoCardsInHand }` is legal
- if the player has a `CannotAct` status, `PassAction { reason: CannotActByStatus }` is legal

A successful pass enters `Action`, consumes the Turn's action, processes and
discards the previous Player's Covered Passive, and emits
`ActionPassed { player, reason }`. It advances to `TurnDraw` unless that Action
ends the game.

### 13. Active Effects Completion

Do not require or event-log an explicit `EndActiveEffects` command.

`ActiveEffects` accepts zero or more Active-Effect Commands. The first valid
Action Command ends `ActiveEffects` and enters `Action`.

Implications:

- successful active effects emit their own events
- failed active-effect validation emits no events
- there is no `ActiveEffectsEnded` event
- replay derives completion from the first Action event in that Turn
- `PassAction` is also an Action Command and therefore enters `Action`

### 14. Public Phase Model

Source basis: official rule 4-2 defines the Turn as Turn Start, Active Effects,
Action, Turn Draw, and Turn End in that order.

Implementation interpretation: expose those five rule stages directly as the
canonical Phase values; a Pending Choice pauses its owning stage rather than
creating a new stage.

Use the official five-stage Turn flow as the canonical public phase model:

```rust
enum Phase {
    TurnStart,
    ActiveEffects,
    Action,
    TurnDraw,
    TurnEnd,
}
```

`ActiveEffects` allows:

- zero or more Active-Effect Commands
- the first accepted Action Command, which enters `Action`

`Action` carries that accepted command through previous-passive handling, all
effects, and every effect-generated Pending Choice. It does not advance to
`TurnDraw` until the Action completes or the game ends. Action Commands include
`PerformFormation`, Profession Change, and `PassAction`.

The Turn Draw discard Pending Choice remains in `TurnDraw`. Neither that choice
nor an effect-generated choice adds a phase.

### 15. Automatic Advancement Through Non-Decision Phases

The public API should stop only when player input is needed.

Non-decision work inside `TurnStart`, `ActiveEffects`, `Action`, `TurnDraw`, and
`TurnEnd` is advanced by the engine automatically while still emitting events.

Suggested API:

```rust
fn apply_command(
    state: &mut GameState,
    command: Command,
) -> Result<Vec<GameEvent>, CommandError>;

fn advance_until_decision(
    state: &mut GameState,
) -> Result<Vec<GameEvent>, EngineError>;
```

Engine stops at:

- `ActiveEffects`, when the current Player may use Active Effects or perform an
  Action
- `Action`, when its Pending Choice requires Player input
- `TurnDraw`, when its discard Pending Choice requires Player input
- the terminal `Finished` state

After `ChooseTurnDiscard`, the engine may automatically process `TurnEnd`,
advance Turn order, run the next Player's `TurnStart`, and stop at the next
decision point.

### 16. Command Handling And Event Application

Tentatively use an event-sourcing command flow:

```rust
fn decide(
    state: &GameState,
    command: Command,
) -> Result<Vec<GameEvent>, CommandError>;

fn apply_event(
    state: &mut GameState,
    event: &GameEvent,
);

fn handle_command(
    state: &mut GameState,
    command: Command,
) -> Result<Vec<GameEvent>, CommandError> {
    let events = decide(state, command)?;
    for event in &events {
        apply_event(state, event);
    }
    Ok(events)
}
```

Command handling validates and decides events. State mutation happens through `apply_event`, so live execution and replay share the same state transition path.

Validation failures return errors, emit no events, and do not mutate state.

The online adapter stores a pending command draft only when
`PerformFormation` produces an `EffectGenerated` choice that suspends its
owning `Action`. The draft preserves the original command context until the
choice continuation completes. A Turn Draw discard choice belongs to
`TurnDraw`, so it must not create or continue the preceding Action Command
draft.

### 17. Formation Matching

Use data-driven formation patterns where practical, with custom matcher hooks for formations that cannot be represented cleanly as data.

Suggested model:

```rust
enum FormationPattern {
    ExactElements(Vec<Element>),
    CountByElement(Vec<(Element, u8)>),
    SequentialGenerating { length: u8 },
    SequentialOvercoming { length: u8 },
    Custom(FormationMatcherId),
}

struct FormationDef {
    id: FormationId,
    name: String,
    category: FormationCategory,
    pattern: FormationPattern,
    effect_id: EffectId,
}
```

`SequentialGenerating` means a continuous five-element generating sequence, such as `木 -> 火 -> 土`.

`SequentialOvercoming` means a continuous five-element overcoming sequence, such as `木 -> 土 -> 水`.

For player convenience, formation matching does not require submitted cards to be in sequence order. The matcher should treat submitted cards as a set/multiset and determine whether any valid ordering satisfies the pattern.

Matcher logic only decides whether cards form a valid formation. Effect planning/resolution is dispatched separately through `effect_id`.

### 18. Formation Declaration

If the same cards can match multiple formations, the player or upper layer must explicitly declare which formation is being performed.

```rust
Command::PerformFormation {
    player: PlayerId,
    formation_id: FormationId,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
}
```

The engine validates that the submitted cards can form the declared `formation_id`. It does not infer or auto-select a formation for the player.

This keeps replay, UI behavior, and AI behavior deterministic when multiple legal interpretations exist.

#### 18.1 Formation Commitment And Completion

Source basis: official rules 5-2 and 5-3 display an accepted Formation before
its effects resolve and Discard the Formation Cards after ordinary completion;
rule 5-4 instead leaves a performed Passive Spell covered for the next Player's
Action.

Implementation interpretation: each Player owns one Formation Area and
successful Formation validation commits the physical Cards to it before any
incoming Counter Effect is processed.

Complete all `PerformFormation` validation before committing any state. When
validation succeeds, emit `FormationCommitted` to atomically move every
submitted physical Card from the performing Player's hand into that Player's
Formation Area and enter `Action`. Attacks and immediate Spells commit face-up;
covered Passive Spells commit face-down.

Commitment is irreversible within that Action. A Counter Effect, prevention,
or no-effect result does not return the Cards to hand. Effect-generated Pending
Choices suspend the same `Action` while the committed Formation remains in its
area.

After a non-passive Formation and all of its sequential choices and effects
finish, emit `FormationCardsDiscarded` to move its physical Cards from the
Formation Area directly to their applicable origin Discard Pile or Piles, then
enter `TurnDraw`. A covered Passive instead emits `PassiveCovered`, remains
face-down in its Formation Area, and enters `TurnDraw`. Do not add an abstract
`FormationUseCompleted` event.

If the Action emits `GameEnded`, neither of those ordinary completion paths
runs. The committed Formation stays in the Formation Area because the rules
stop further processing rather than performing terminal cleanup.

### 19. Attack Base Damage Target

Attack formation base damage always targets the previous player (`上家`) as resolved from turn order.

Players do not declare the base damage target for attacks. The engine derives it:

```rust
let damage_target = previous_player(current_player, turn_order);
```

`declared_targets` on `PerformFormation` are reserved for extra effects that require choices. The engine validates target count and target type against the declared `FormationDef`.

### 20. Declared Targets

`declared_targets` stores only concrete player choices:

```rust
enum TargetDecl {
    Player(PlayerId),
    Team(TeamId),
    Card(CardInstanceId),
}
```

Rule-derived semantic targets such as previous player, next player, self, own side, or opponent side are not submitted by players. They are resolved by formation/effect logic from current state.

This keeps the event log focused on actual player choices while allowing rule-defined targets to remain deterministic projections of state.

### 21. Previous-Turn Formation Query

Source basis: rule 5-2.4b conditions Five-Element interaction on the Previous
Player having performed a Five-Element Attack during the immediately completed
Previous Turn. Basic rule 2-2.2 and 2-2.4 give Metamorphosis the same turn scope
for its copy target. Jianghu rule 4-7 uses that scope for Water-Dotting Fan.

Implementation interpretation: `last_formation_by_player` is retained as replay
state, but these three rules query it only when `resolved_turn == turn_number - 1`.
No record qualifies after Pass, Profession Change, or a turn without a Formation.
Triggering an existing Formation's effect at Turn Start does not create a new
Formation use. A covered Passive belongs to the turn when it was covered.

The last-formation state stores the performed formation identity and its resolved
category/effect separately. When Metamorphosis performed during the immediately
completed Previous Turn copied a Five-Element Attack, Five-Element interaction
reads that copied element while the Formation identity remains `幻化`.

Five-element attack attributes are not sealed. If a legal five-element attack is performed, its element remains available for five-element interaction tracking even if other parts of the action are affected by defensive effects.

Damage prevention, shield absorption, or other defensive effects do not change
the resolved formation category.

Validation failure still does not update it because no game event occurred.

### 22. Attack Point Breakdown

Represent five-element point modification explicitly instead of hiding it in final damage.

```rust
enum ElementInteraction {
    Generating,
    Overcoming,
    Same,
    None,
}

struct AttackPointBreakdown {
    base_points: i32,
    interaction: ElementInteraction,
    damage_transform: DamageTransform,
    final_amount: i32,
}

enum DamageTransform {
    NormalDamage,
    HealTarget,
    DoubleDamage,
    HalfDamageRoundUp,
}
```

An Attack Resolution includes the point breakdown together with all of its
resolved consequences:

```rust
struct AttackResolution {
    outcome: ActionOutcome,
    point_breakdown: AttackPointBreakdown,
    damage: AttackDamageResolution,
    additional_effects: ResolvedAttackEffects,
}
```

This supports deterministic replay, focused tests, and UI/debug display of how final damage was derived.

### 23. Five-Element Interaction Rules

Five-element interaction is resolved from the current attack element against the
element of the Five-Element Attack performed by the Previous Player during the
immediately completed Previous Turn.

Official relationships:

- generating order: `金 -> 水 -> 木 -> 火 -> 土 -> 金`
- overcoming order: `金 -> 木 -> 土 -> 水 -> 火 -> 金`
- same element: resisting/same (`相抵`)

When the current attack element generates the previous player's last attack element, the attack does not deal damage and instead heals the target for the attack amount.

When the current attack element overcomes the previous player's last attack element, damage is doubled.

When both elements are the same, damage is halved and rounded up.

Examples:

- previous `金`, current `火`: overcoming, damage doubled
- previous `金`, current `土`: generating, heal target
- previous `金`, current `金`: same, damage halved and rounded up
- previous `金`, current `木` or `水`: no interaction

If the target has a shield, skip five-element interaction entirely.

Five-element interaction changes the damage/healing result, not the attack points.

### 24. Shield Ownership And Damage Absorption

Shields are attached to players, not teams.

```rust
shield_by_player: HashMap<PlayerId, Shield>

struct Shield {
    amount: i32,
}
```

Attack base damage still targets the previous player. HP damage resolves to that player's team, but shield lookup happens on the target player.

Shields have no duration. They persist until depleted or explicitly changed by another effect.

If the target player has a shield:

- skip five-element interaction
- apply attack damage to the shield
- excess damage does not pierce through to player/team HP

Physical attacks deal double damage to shields. Special attacks and five-element
attacks use their otherwise resolved damage amount.

The shield absorbs the attack as a separate defensive layer, even if the incoming damage exceeds the shield amount.

When shield amount reaches 0 or lower, remove it immediately.

If a player who already has a shield gains a new shield, the new shield amount replaces the old amount. Shield values do not stack.

### 25. Passive Action Modifications

Passive spells do not hard-code a single "counter means cancel everything" behavior in the core pipeline.

Each passive effect returns one or more action modifications:

```rust
enum ActionModification {
    Continue,
    CancelAction { reason: CancelReason },
    PreventDamage,
    ModifyDamage(DamageModifier),
    PreventExtraEffects,
}
```

Passive resolution flow:

1. At next player's action start, flip the previous player's covered passive.
2. The passive effect checks whether it applies to the incoming action.
3. The effect returns action modifications.
4. The incoming action resolution applies those modifications.

Within an Attack Resolution, damage and every attached effect without another
specified timing are one atomic, simultaneous result from the Players'
perspective. They belong to one `AttackResolved` event so implementation,
replay, and Public Views cannot invent an observable order. An explicitly timed
effect remains outside that event, while a required intermediate choice is
collected before emitting it.

Countershock (`反震`) splits an incoming attack before defensive layers absorb
damage. The attacking side receives its reflected share directly. The defending
player's share then resolves against that player's shield, if present, under the
normal shield rules. A shield must not suppress Countershock's split. If either
side reaches 0 HP while applying the split, resolution continues until both shares
have been applied; game-over evaluation uses the fully resolved state and may
therefore produce a draw.

Five-element interaction determines the attack's result mode before Countershock
distributes its amount. Countershock divides the resulting amount but does not
change that mode. Therefore, when generating interaction makes an attack restore
HP, both the attacking and defending sides restore their respective shares.
Overcoming, same-element, and unrelated interactions likewise apply their modified
amount before it is divided.

Fractional formation points always round up. Countershock calculates each side's
share independently with ceiling division, so an attack amount of 7 produces 4
points for the attacking side and 4 points for the defending side.

Seal (`封印`) applies only to spells (`術式`). Do not model it as a broad "spell or effect" cancellation rule.

If seal flips against an incoming attack, it is discarded with no effect because the incoming action is not a spell.

```rust
GameEvent::PassiveFlipped {
    owner: PlayerId,
    incoming_player: PlayerId,
    passive_id: FormationId,
    outcome: PassiveOutcome::NoEffect {
        grounds: vec![PassiveNoEffectGround::NotASpell],
    },
}
```

Defense (`防禦`) applies only to incoming attacks. If it flips against an incoming spell, pass action, or other non-attack action, it is discarded with no effect.

Defense prevents only the attack's damage. Other effects of the performed attack
still resolve; in particular, Five Streams Unite still grants its turn-draw bonus.

If seal applies to an incoming passive spell cover action, it does not immediately discard or reveal the incoming covered passive cards.

Instead, the incoming passive remains covered and is marked sealed. When that covered passive later flips at its own trigger timing, it resolves as no effect and is then discarded normally.

This prevents a covered passive from being revealed early and preserves the next player's ability to make decisions, such as choosing Metamorphosis (`幻化`), without knowing the covered card identity.

The sealed covered passive uses the same public view behavior as a normal covered passive. The engine stores the sealed marker internally for later resolution, but public/player views do not expose a separate sealed marker beyond the normal covered-card visibility rules.

Each Player's Formation Area can contain at most one Formation. Under normal
Turn flow, a Player's Covered Passive flips during the next Player's `Action`
before its owner can commit another Formation.

If a command attempts to commit a Formation while that Player's Formation Area
is already occupied, report an error. Emit no event and do not consume the
Action.

Empty City (`空城`) is a covered passive with no additional action modification.
It still consumes the formation cards and turn action, occupies the player's one
Formation Area, flips at the normal trigger timing, and moves its cards
to discard. Its no-effect outcome is intentional rather than an unknown-passive
fallback. Represent that outcome explicitly as
`PassiveNoEffectGround::EmptyCity`. User-facing records should say only
`空城翻開`; they must not add redundant wording about producing no effect.

Metamorphosis (`幻化`) is a basic formation, not a special standalone action command and not a special formation category/tag.

It should be handled through the normal `PerformFormation` pipeline like other formations. It consumes the turn action, triggers covered passives at the usual next-player action timing, and uses formation category/effect rules to determine whether any passive applies.

A player with a covered passive necessarily used that passive as their
Previous-Turn Formation. Therefore, a next-player Metamorphosis cannot both
trigger that passive and copy an older attack from the same player. Treat that
combination as unreachable rather than adding an interaction rule or test for it.

Metamorphosis preserves its own formation identity and name while copying the
Previous-Turn Formation's resolved category and effect. The last-formation state
must therefore store formation identity separately from the resolved effect plan.
A later Metamorphosis copies that resolved category and effect, so Metamorphosis
chains continue to reproduce the original copied behavior even though every
link is still displayed and recorded as `幻化`.

Metamorphosis does not copy a spell's active/passive type because that type controls
how the formation is performed. Metamorphosis remains an active spell: its two
Earth cards are shown face up and discarded through the active-spell procedure.
When it copies Defense, Seal, Countershock, or another delayed counter effect, the
copied effect still waits for and modifies the next player's action, but it is
public and has no covered cards. Delayed counter state must therefore be modeled
separately from covered-passive card state.

Copying Empty City records Empty City as the resolved effect but creates no
delayed counter because Empty City has no effect to establish.

When the copied effect uses a card-based formula, each Metamorphosis evaluates that
formula from its own two submitted Earth cards. It copies the formula, category,
and effect behavior, not the Previous-Turn Formation's already calculated amount.
For a copied single-card elemental strike, `level + 4` means the submitted
Metamorphosis cards' level sum plus 4; it must not read only one of the two
Earth cards.

Copying Five Streams Unite (`五流歸一`) includes its complete effect. Resolve its
damage from the target's current hand count and grant the Metamorphosis player the
formation's next-draw bonus. The bonus is not limited to directly submitting the
original five-card formation.

Do not add a separate `ChangeClass` command for Metamorphosis. Profession Change
is a distinct Action introduced by the corresponding rule modules.

Formation category is limited to the official two categories:

```rust
enum FormationCategory {
    Attack,
    Spell,
}
```

Passive spells are represented as spells whose `effect_id` points to an `EffectDef` with a delayed/covered plan, not as a separate top-level category.

Do not add separate categories or tags such as `ClassChange`, `PhysicalAttack`, or `ElementalAttack` unless they are needed by a concrete rule. Differences such as elemental attack interactions, passive spell covering, or Metamorphosis behavior should be represented by effect definitions under the official `Attack`/`Spell` category model.

Use `effect_id + EffectDef.plan` rather than `FormationBehavior`:

```rust
struct EffectDef {
    id: EffectId,
    plan: EffectPlanDef,
}

enum EffectPlanDef {
    Attack(AttackPlanDef),
    ImmediateSpell(ImmediateSpellPlanDef),
    CoveredPassive(CoveredPassivePlanDef),
}
```

`FormationDef` identifies what the formation is and how it is matched. `EffectDef.plan` identifies how the formation executes.

Attack plan details may distinguish attack subtypes without changing the official formation category:

```rust
enum AttackPlanDef {
    Elemental { element: Element, points: PointFormula },
    Physical { points: PointFormula },
    Special { points: PointFormula, special: SpecialAttackId },
}
```

Five-element interaction only applies to `AttackPlanDef::Elemental`. Defense applies to all `EffectPlanDef::Attack` plans.

Attack points use formulas instead of only fixed values:

```rust
enum PointFormula {
    Fixed(i32),
    PerCard { points_per_card: i32 },
    SumCardValues,
    Custom(PointFormulaId),
}
```

Fixed point attacks use `PointFormula::Fixed`. Variable point attacks add a formula resolver without changing the formation schema.

Custom matchers, point formulas, and effect resolvers must be deterministic pure resolvers.

Example:

```rust
trait PointFormulaResolver {
    fn compute(&self, ctx: PointFormulaContext) -> Result<i32, RuleError>;
}
```

Allowed:

- read formation cards
- read card definitions
- read the current state snapshot
- return validation results, computed points, or effect intents

Not allowed:

- mutate `GameState`
- move cards between zones directly
- append `GameEvent` directly
- use non-deterministic RNG, wall-clock time, or IO

Any randomness or player choice must be represented explicitly through commands and events.

Immediate spell, passive spell, and attack effect resolvers all return declarative intents. The pipeline converts intents into events, and `apply_event` mutates state.

```rust
struct EffectIntent {
    hp_changes: Vec<HpChangeIntent>,
    shield_changes: Vec<ShieldChangeIntent>,
    status_changes: Vec<StatusChangeIntent>,
    card_moves: Vec<CardMoveIntent>,
    action_modifications: Vec<ActionModification>,
    pending_choices: Vec<PendingChoiceIntent>,
}
```

Formation-use baseline zone movement is owned by the pipeline, not by
individual effect resolvers. It is emitted as `FormationCommitted` before
effects and `FormationCardsDiscarded` after ordinary face-up completion.

Effect intents describe additional consequences only, such as drawing cards, discarding selected cards, changing shields, changing statuses, modifying actions, or requesting further choices.

Effect-generated pending choices are required in the first version because base formations use them.

Only one pending choice may be active at a time. When an effect creates a pending choice, the engine stops until the required player submits the corresponding choice command.

Effect choices and randomness requests carry one serializable resolution owner. The input
objects themselves remain only input requirements:

```rust
enum PendingResolution {
    TurnDrawDiscard,
    ChaosReturnTwo,
    // ... one explicit variant for every paused rule flow
}
```

`PendingResolution` must be serializable and replay-safe. It cannot contain closures, trait
objects, borrowed references, or non-deterministic runtime state.

Pending input invariant:

```rust
pending_choice: Option<PendingChoice>
pending_randomness: Option<PendingRandomness>
pending_resolution: Option<PendingResolution>
```

At most one input requirement and exactly its matching `PendingResolution` may exist at a
time. The public view never exposes `PendingResolution`.

If a resolution needs multiple choices, it presents them sequentially. After an answer or a
randomness result, the engine resumes from `PendingResolution` and may request the next input.

Creating and answering an effect-generated Pending Choice are both Game Events:

```rust
GameEvent::ChoiceRequested {
    choice: PendingChoice,
    resolution: PendingResolution,
}

GameEvent::ChoiceMade {
    choice_id: ChoiceId,
    player: PlayerId,
    selection: ChoiceSelection,
}
```

`ChoiceRequested` must include enough serialized data to reconstruct both the pending choice
and its resolution owner during replay. `RandomnessRequested` follows the same pattern.

Turn Draw retains its specific Card-movement events while using the ordinary
Pending Choice lifecycle. `CardsDrawnForTurnDiscardChoice` creates the Pool,
then `ChoiceRequested` creates its Pending Choice. The answering Command emits
`ChoiceMade` before `TurnDrawResolved`, which records the answer's Discard and
kept-Card movements atomically. No Player input or replay state exists between
those two movements inside `TurnDrawResolved`.

Canonical events may contain hidden information required for deterministic replay, including
hidden card ids, complete choice options, and serialized resolutions.

External event feeds must be viewer-filtered:

- the choice player can see the options they are allowed to choose from
- non-choice players see only public information, such as that a player is making a choice
- replay uses canonical events, never filtered public events

Radiance (`光芒`) also inspects the next player's hand when it resolves. Record a
canonical snapshot of that hand at resolution time. Only the formation player may
see the snapshot in the filtered event; the inspected player, other players, and
observers receive only the public fact that Radiance resolved. This is a snapshot,
not ongoing permission to observe later hand changes. Keep that snapshot available
to the formation player in their private event history so reconnect and replay do
not lose information they already inspected.

### 26. Game Over

Source basis: official rule 7-3 ends the Game immediately when its end
condition is established and stops unfinished effect processing.

Implementation interpretation: finish only the simultaneous semantic
resolution currently being applied, record an explicit terminal conclusion,
and run no later effect or cleanup step.

The game ends when a Team's HP reaches 0 or lower or another enabled rule
produces a direct terminal outcome. Store an independent Game Conclusion:

```rust
enum GameStatus {
    Ongoing,
    Finished { conclusion: GameConclusion },
}

struct GameConclusion {
    outcome: GameOutcome,
    causes: Vec<GameEndCause>,
}

enum GameOutcome {
    Winner(TeamId),
    Draw,
}

enum GameEndCause {
    TeamHpDepleted { teams: Vec<TeamId> },
    DirectVictory { rule: RuleRef, team: TeamId },
}

GameEvent::GameEnded {
    conclusion: GameConclusion,
}
```

`causes` is non-empty by invariant; the concrete Rust representation may use a
non-empty collection type. `GameEnded` explicitly records the final outcome and
reason instead of asking replay or presentation to infer them from an earlier
damage, victory, Formation Area, or Last Formation event.

2-player mode still uses Teams: each Player belongs to their own single-player
Team. If one Team's HP reaches 0 or lower, the other Team wins.

If the same simultaneous resolution causes all opposing Teams to reach 0 or
lower, the result is `GameOutcome::Draw`. Apply every delta in that simultaneous
semantic resolution, including rule-defined automatic responses that precede
Game Outcome evaluation, before deciding the conclusion.

Emit `GameEnded { conclusion }` immediately after that evaluation and make it
the last canonical event. Do not start a later sequential effect, discard a
Formation merely as cleanup, enter `TurnDraw`, or enter `TurnEnd`. The Card
zones already established remain unchanged, so a terminal current Formation
can remain in its Player's Formation Area.

The finished-table presentation is derived separately from the recorded
decision that contains `GameEnded`, after applying the viewer's event
redaction. A Formation commit or resolved Attack can provide its player,
Formation, and cards; discard retrieval and automatic effects have their own
presentations. This display is the terminal decision, not an end cause, and it
never falls back to `last_formation_by_player`.

Once `GameStatus::Finished` is reached, gameplay commands are rejected.

HP state is clamped to 0 and never stored as a negative value.

```rust
new_hp = max(0, old_hp - damage)
```

Team HP is also capped at that match's initial HP. Every recovery source uses the
same cap, including Generating Formation, Return to Origin, generating elemental
attacks, and generating attacks divided by Countershock.

Every HP change fact should preserve enough detail for audit/debug, whether it
is nested in an `AttackResolved` event or emitted by a non-Attack source:

```rust
struct HpChangeDelta {
    team: TeamId,
    old_hp: i32,
    delta: i32,
    new_hp: i32,
    effective_delta: i32,
}
```

`new_hp` is clamped. `effective_delta` records the actual state change after clamping.

### 27. Initial Deal

Initial hands are dealt by the engine from the prepared deck order and emitted as events.

Rule:

- first player receives 4 cards
- all other players receive 5 cards

Suggested events:

```rust
GameEvent::DeckPrepared {
    deck_order: Vec<CardInstanceId>,
}

GameEvent::CardsDealt {
    player: PlayerId,
    cards: Vec<CardInstanceId>,
    reason: DealReason::InitialHand,
}
```

`GameSetup` should define players, teams, turn order, HP setup, and ruleset. It should not directly contain starting hands.

Initial HP is provided by `GameSetup`, not hard-coded by the engine.

```rust
struct GameSetup {
    players: Vec<PlayerSetup>,
    turn_order: Vec<PlayerId>,
    hp: HashMap<TeamId, i32>,
    ruleset: RulesetId,
}
```

Official default builders may populate HP:

```rust
GameSetupBuilder::official_2p(players)
GameSetupBuilder::official_team(players, teams)
```

Tests and variants may provide explicit HP values.

### 28. Team Setup Validation

Team mode setup must be validated by the engine, even if helper builders generate legal setups.

Validation rules:

- at least 4 players
- exactly 2 teams
- every player belongs to a team
- both teams have the same number of players
- `turn_order` contains every player exactly once
- adjacent players in `turn_order` cannot belong to the same team
- circular adjacency is checked, so the last and first players also cannot belong to the same team

Setup builders may help produce valid alternating seating, but invalid setup data must still be rejected.

### 29. Card Instances And Definitions

Zones store card instances, not card definitions.

```rust
struct CardInstance {
    id: CardInstanceId,
    def_id: CardDefId,
}

struct GameSetup {
    card_instances: HashMap<CardInstanceId, CardDefId>,
}
```

Decks, hands, Discard Piles, Formation Areas, the Turn Draw Pool, and other Card
zones store `CardInstanceId`.

Rules and matchers resolve `CardInstanceId -> CardDefId -> CardDef` through setup data and the card definition registry.

The instance-to-definition mapping is immutable after game setup. Replay relies on stable `CardInstanceId` values.

### 30. Card Definition Shape

Use the minimal card definition shape for the first version:

```rust
struct CardDef {
    id: CardDefId,
    name: String,
    element: Element,
    level: u32,
}
```

`level` is a stable printed-card attribute used by base attack point formulas such as `level + 4` and `sum(level) * N`.

Do not add `card_kind` or optional elements until a concrete rule requires non-element cards.

### 31. Base Formation Registry

Create the full registry for the 25 base formations in the first implementation.

Each base formation should have:

- `FormationDef`
- `EffectDef`
- matcher coverage

Effect resolvers may be implemented incrementally. A known but unimplemented effect may return `RuleError::EffectNotImplemented(effect_id)` until implemented.

This lets validation, schema, and matcher tests stabilize before every effect is complete.

An unimplemented effect is not a validation failure.

If the player submits a legal command for a known formation whose effect resolver is not implemented, return an implementation error and do not emit events or mutate state.

```rust
enum CommandError {
    Validation(ValidationError),
    RuleImplementation(RuleImplementationError),
}
```

`EffectNotImplemented(effect_id)` belongs to `RuleImplementationError`.

### 32. Status Duration

Use explicit expiry timing for status durations:

```rust
enum Duration {
    UntilTurnStart { player: PlayerId },
    UntilTurnEnd { player: PlayerId },
    UntilTurnEndNumber { player: PlayerId, turn_number: u64 },
    Permanent,
}
```

Do not include `UntilNextAction` in the first version. Next-Action Passive
behavior is modeled by the Covered Passive in a Player's Formation Area, not by
generic status duration.

Avoid generic `remaining_turns` / `remaining_rounds` counters as the core model because they are ambiguous in multiplayer and team mode. Rule resolvers can translate rule text into explicit expiry timing.

Rules 6-1 and 6-2 give the presentation units distinct owner-relative timing:
a Turn elapses at the affected Player's Turn End, while a Round elapses at that
Player's Turn Start. Therefore Web presentation counts only that Player's
matching boundaries. It must not subtract the current global `turn_number`
from an expiry ordinal and label that difference as remaining Turns. A status
with `UntilTurnStart` displays its duration in Rounds; 天陽罡 uses this duration
because its published duration is one Round.

Status expiration is event-logged:

```rust
GameEvent::StatusExpired {
    status_id: StatusId,
    owner: StatusOwner,
    expired_at: ExpiryTiming,
}
```

`advance_until_decision` checks expiry at `TurnStart` and `TurnEnd` timing. Expired statuses are removed through events, not silent state mutation.

### 33. Replay Versus Verification

Pure replay only applies recorded events. It does not re-run command decision logic or recompute automatic events.

Modes:

- live execution: commands and automatic advancement decide events, then apply them
- replay: apply the canonical event log exactly as recorded
- verification: optionally rerun commands/automatic advancement and compare produced events with the canonical event log

The canonical event log already contains automatic events such as initial deal,
Discard Shuffles, choice requests, and status expiration. Recomputing them
during replay could duplicate events or diverge across ruleset versions.

Canonical `GameEvent` and `PendingChoice` payloads participate in replay verification. Data needed only by a client, such as an effect choice's required card count or a terminal-decision display, belongs in the Web projection and is derived from canonical records and public state.

### 34. Event Granularity

Source basis: official rules 2-5.3c and 5-2.4i make an Attack's damage and every
attached effect without another specified timing simultaneous and unordered.

Implementation interpretation: represent that complete Attack Resolution as
one atomic `AttackResolved` event. A sequence of otherwise semantic events
would still impose a canonical before-and-after relationship that the rule
explicitly denies.

Use semantic events that include explicit replayable deltas.

Avoid events that are too vague to replay without recomputing rules:

```rust
GameEvent::FormationPerformed { formation_id: FormationId }
```

Do not add an abstract `FormationUseCompleted` event. Normal completion is the
next rule-significant fact—`FormationCardsDiscarded` or `PassiveCovered`—and a
terminal Formation instead ends with `GameEnded`.

Also avoid reducing the whole log to only low-level mutation events with no domain meaning:

```rust
GameEvent::CardMoved { ... }
GameEvent::HpChanged { ... }
GameEvent::StatusAdded { ... }
```

Prefer semantic events with concrete deltas:

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

This keeps the log understandable while allowing `apply_event` to mutate state
directly from recorded facts rather than recomputing rule outcomes. Baseline
Formation Card movement is represented by `FormationCommitted` and
`FormationCardsDiscarded`, not duplicated inside each effect event.

`AttackDamageResolution` contains every HP and Shield consequence of the
Attack's damage, including split or reflected shares. `ResolvedAttackEffects`
contains every replayable delta belonging to attached effects at the same
timing, including additional Team HP or Shield changes, Status application,
Card or Environment changes, or Turn Draw bonuses as applicable. Applying
`AttackResolved` applies its complete `AttackResolution` atomically.

Field layout and any collections inside `AttackResolution` use a stable
canonical serialization order only. Their position never represents a rule
processing order.

Do not emit separate `HpChanged`, `ShieldChanged`, `StatusAdded`, `CardsMoved`,
or module-specific effect events for consequences that belong to the same
Attack Resolution. Such events remain valid for non-Attack sources and for
Attack effects whose rules explicitly specify another timing.

When an attached effect requires Player input, record its Pending Choice and
continuation first without applying damage or another simultaneous consequence.
After the final answer, emit the complete `AttackResolved` event. Game Outcome
evaluation follows that event, so a lethal damage delta never suppresses
another consequence in the same Attack Resolution.

This changes a persisted event contract. Implementation requires an explicit
record migration that folds legacy same-timing Attack consequence events into
the corresponding `AttackResolved`; snapshots may then be rebuilt.

`TurnDrawResolved` explicitly records both the chosen discard and all kept
Cards. Applying that one event moves the discard from the Turn Draw Pool to its
applicable origin Discard Pile before moving the kept Cards from the Pool to
hand; there is no canonical intermediate state.

A single command or automatic advancement may emit multiple semantic events.

Example attack resolution may emit:

```rust
GameEvent::FormationCommitted { ... }
GameEvent::PassiveFlipped { ... }
GameEvent::AttackResolved { ... }
GameEvent::FormationCardsDiscarded { ... }
GameEvent::CardsDrawnForTurnDiscardChoice { ... }
```

If `AttackResolved` produces a terminal conclusion, the sequence emits
`GameEnded { conclusion }` immediately after it and stops; it does not emit
`FormationCardsDiscarded`, `CardsDrawnForTurnDiscardChoice`, or any later event.

Do not force all results into one large `CommandResolved` event. Event boundaries should follow meaningful domain moments, especially where resolution can pause for a pending choice.

Wrap domain events in recorded-event metadata:

```rust
struct RecordedEvent {
    sequence: u64,
    source: EventSource,
    event: GameEvent,
}

enum EventSource {
    Command { command_id: CommandId, player: PlayerId },
    Automatic { reason: AutomaticReason },
    Setup,
}
```

`GameEvent` remains focused on domain facts. `RecordedEvent` carries ordering and source metadata for debugging, UI acknowledgement, networking, and audit.

Replay relies on `sequence` and `event`, not on `command_id`.

Do not persist a separate command log in `GameRecord`.

`RecordedEvent.source` is enough to correlate accepted commands with their emitted events. Validation failures are API errors and are not part of the canonical game record.

### 35. Error Taxonomy

Use three command error layers:

```rust
enum CommandError {
    Validation(ValidationError),
    RuleImplementation(RuleImplementationError),
    EngineInvariant(EngineInvariantError),
}
```

Meanings:

- `Validation`: player command is illegal and can be shown to UI
- `RuleImplementation`: rules module or effect implementation is incomplete/incorrect
- `EngineInvariant`: an impossible state or invariant violation was reached

Examples of `EngineInvariant`:

- Deck plus its applicable Discard Pile cannot satisfy a required draw
- duplicate pending choice
- duplicate covered passive in a state where turn flow should have prevented it
- zone ownership inconsistency

Command and automatic advancement errors are atomic.

If `decide` fails with any `CommandError`, no events are emitted and no state is mutated.

`handle_command` applies events only after the full event list has been successfully decided.

`apply_event` is a pure projection over canonical events. It trusts the event log and does not rerun gameplay validation or invariant checks.

Validation and invariant checks happen before events are recorded, or in explicit verification tooling.

Because `apply_event` trusts canonical events, it should be infallible:

```rust
fn apply_event(state: &mut GameState, event: &GameEvent);
```

### 36. Five Directions Legend Rule Module

Implement the official
[Five Directions Legend rules](https://www.cfecards.org/rule/latest/field) as
one independently configurable Advanced Rule Module. Enabling the module adds
all five Sacred Beasts, one shared Environment, and Void Meridian-Severing
Technique; none of those features has a separate toggle.

The Environment is absent at game start and holds at most one `Element`.
`EnvironmentTransferred` is a semantic canonical event emitted after every
Sacred Beast attack resolves, including a transfer whose previous and resulting
elements are equal. The attack uses the previously existing Environment. A
lethal attack still completes its Environment Transfer before the Formation Use
and Game Outcome are finalized.

Each Sacred Beast is an 81-point elemental Attack made from five Cards of its
element:

- East Azure Dragon: Wood
- West White Tiger: Metal
- South Vermilion Bird: Fire
- North Black Tortoise: Water
- Center Yellow Serpent: Earth

A Sacred Beast ignores Formation effects. Covered Passives and public Counter
Effects still trigger and are consumed at their normal timing, but resolve with
`NoEffect` against it. General Shield rules still apply because a Shield is a
separate game element rather than an incoming Formation effect.

Environment Effects apply to all Players:

- damage matching the Environment's element is doubled and stacks with
  five-element interaction
- damage whose element generates the Environment's element becomes HP recovery
- simultaneous overcoming doubles that recovery, while same-element
  interaction halves it and rounds up
- the Attack breakdown records the Environment Effect separately from
  `ElementInteraction` and the final amount

A Shield receives an incoming Attack before its owner. Five-element interaction
does not apply while the Shield receives the Attack. An Environment Effect that
would convert damage into HP recovery therefore does not apply because a Shield
has no HP, while same-Environment damage doubling still changes the damage
received by the Shield.

Each Environment makes two named Formations ineffective:

- Metal: Defense and Barrier
- Wood: Metamorphosis and Chaos
- Water: Countershock and Shock Burst
- Fire: Weapon and Radiance
- Earth: Seal and Return to Origin

An ineffective Formation remains legal to perform, consumes the action
opportunity, and follows its normal Card movement procedure, but its effect
resolves with `NoEffect`. Attacks and Active Spells emit
`FormationEffectIgnored` with the current Environment as the reason; Covered
Passives retain the ineffectiveness established when performed and report it
in their later flip outcome, rather than reevaluating a changed Environment.
See section 45 for the confirmed timing and current implementation gap.
Invalidation checks the
performed Formation's identity, not an effect copied by Metamorphosis. Existing
persistent state is not removed; for example, an ineffective Barrier does not
replace or clear an existing Shield.

Void Meridian-Severing Technique is an Active Spell formed from three same-level
Cards. With no Environment it performs normally but does not change HP. When an
Environment exists and the Spell is not otherwise made ineffective, it clears
that Environment and changes each Team's HP by -20 exactly once, regardless of
Player count. Shield values are irrelevant. Record the Environment Clearing and
all Team HP deltas in one `EnvironmentCleared` event, apply every delta, and
only then evaluate the Game Outcome so simultaneous defeat produces a Draw.

Official setup HP follows the current rulebook instead of the previous
20-point placeholder:

- Base-only two-player and four-player games use 100 and 150 Team HP
  respectively
- games with an Advanced Rule Module use 200 and 250 Team HP respectively

The module configuration, current Environment, canonical events, attack
breakdowns, and Game Outcome must replay deterministically and project through
Public State View, Public Event Feed, Web DTOs, and the online battlefield.

### 37. Hero Schools Rule Module

Implement the official
[Hero Schools rules](https://www.cfecards.org/rule/latest/hero) as one
independently configurable Advanced Rule Module, including all Professions,
Profession Abilities, Profession Formations, Profession Changes, and Void
Reversion Technique. New official games enable the complete module by default; the Web
room configuration exposes only one Hero Schools toggle.

Every Player starts without a Profession. A Profession is acquired only through
a successful Profession Change, including First Wanderer.

Profession Change is a distinct Action Command rather than a Formation Use. It
validates the declared Profession, prerequisite Profession, and submitted Card
Instances; then it enters `Action` and processes the Previous Player's Covered
Passive. The selected Cards remain canonically in hand during that preceding
passive handling. If the change proceeds, one semantic `ProfessionChanged`
event atomically records both the new Profession and direct movement of those
Cards from hand to their applicable origin Discard Pile or Piles. There is no
temporary Profession-Change Card zone. The command consumes the current
Player's Action. Do not represent Profession Change as `PerformFormation` or
confuse it with Metamorphosis.

Game State stores only each Player's current `ProfessionId`, or no Profession,
rather than copying the Profession's effective abilities into state.
`ProfessionChanged` records the previous and resulting Profession identities.
The official Profession catalog derives effective abilities through explicit
inheritance links for the five Schools. Immortal and Saint have no inheritance
link, so changing to either removes the previous Profession's abilities.

All Activated Profession Abilities use one
`ActivateProfessionAbility` Active-Effect Command and one shared allowance per
Player turn. A successful activation consumes that allowance without consuming
the action opportunity. Validation failure consumes neither. Profession Change
does not reset the allowance, so acquiring a different Activated Profession
Ability during the same turn never permits a second activation.

`rules::profession::activated` is the single lifecycle owner for these
Commands. Its Base-facing interface has only offer generation and activation.
It resolves the external string ID once into one nested closed enum, delegates
to statically dispatched Hero Schools, Jianghu, Confluence Generation, or Dark
Glimmer providers, and rejects unknown IDs without prefix fallback or runtime
registration. The shared module validates the effective Profession Ability
Set, Temporary Ability Loss, the shared turn allowance, and a private validated
Action Card Selection in that order.

Each provider returns an offer plan with one completion contract and declarative
player-facing metadata. The same offer authority is rebuilt for Command
validation. Target Card, declared element, and declared level must each be
absent, equal to the offered fixed value, or belong to the offered required
values exactly; unrelated fields and fallback omissions are Validation
Failures. Dark Spirit therefore accepts only the selected Card's offered
effective element and cannot change that element through a direct Command.

Effect planning uses one scratch projection. The common builder records exactly
one `ProfessionAbilityActivated` event first, then ordered provider
consequences, then at most one final Choice or Randomness continuation whose
medium matches its `PendingResolution`. Providers cannot record the activation
event themselves. This permits a cost to enter its origin Discard Pile before
Deck Supply is planned. Revelation and Azure Cloud Step therefore include the
just-Discarded cost Card in a required Discard Shuffle.

Azure Cloud Step uses two typed continuations. An insufficient Deck waits on
the Randomness-only `JianghuAzureCloudStepDraw`; after the Discard Shuffle it
draws two Cards and creates the existing Choice-only
`JianghuAzureCloudStepReturnOne`. Do not migrate or reinterpret an older invalid
record that used the Choice resolution as its Randomness resolution.

Formation Proficiencies add Player-scoped alternative matchers to an existing
Formation. They do not register duplicate Formations or replace the original
Formation definition. A Formation Use matched through a Proficiency retains the
original Formation identity, category, effect, point formula, card movement,
and canonical event semantics. The alternative matcher is available only while
the performing Player's current Profession grants it; Public Formation queries
may label the matching Proficiency as presentation metadata.

Formation matching returns distinct `FormationMatchOption` values whenever the
same submitted Card Instances have multiple legal role assignments that change
the result. `PerformFormation` must declare one of those options rather than
letting the engine maximize or otherwise choose an outcome. The accepted
Formation event records the resolved role binding and calculation. Formation
queries expose the legal options and result previews; a single unambiguous
option requires no additional Web interaction.

Void Reversion Technique resolves as one atomic semantic
`VoidReversionResolved` event. It records the performing Player's Team HP delta
of -20, every Profession actually broken, every Legendary Profession retained
because the Cards were below level three, and the Formation's Card movement.
Apply all deltas before evaluating the Game Outcome. Reaching zero HP from the
cost does not stop Profession Breaking or expose an intermediate state.

Automatic Profession Abilities and Formation Proficiencies integrate through
typed Rule Module hooks at explicit rule stages. Hooks may extend
Formation matching, modify costs or Attack calculation, determine immunity,
modify Counter Effect applicability, or return post-Formation intents. They
return declarative values for the shared pipeline and never mutate Game State
or emit Game Events directly. Do not encode Profession Abilities as generic
Status Effects or scatter Profession identity checks through Base resolvers.
Activated Profession Ability providers instead return effect plans for a new
Player-selected Active Effect; they are not stage hooks and do not bypass the
shared activation lifecycle.

Cross-module Attack resolution uses one explicit stage order:

1. Apply legal Card interpretations and select a Formation Match Option.
2. Compute Attack Points, including Profession point modifiers.
3. Evaluate Attack-Point qualifications such as Windwalking and Star Summoning.
4. Resolve applicable Counter Effects.
5. Apply five-element interaction, Environment Effects, and Shields to obtain
   the final damage or recovery result.
6. Resolve post-Formation Profession intents.

Windwalking's 15-point threshold therefore reads Attack Points before
Environment damage modification. Canonical Attack events record Attack Points
separately from the final result.

Physical-Card preparations and virtual Formation components use different
domain concepts. Blazing Yang Art and Dark Spirit create a serializable
physical Card Interpretation for one Card Instance. Illusion and Phantasm
instead discard two physical Cards and create a `VirtualFormationCard` with a
fixed source ability, element, and level; neither ability targets or
reinterprets a third Card Instance.

The physical preparation's ability identity and interpreted facts are public,
while its target Card Instance remains visible only to its owner until ordinary
Card movement makes it public. Illusion and Phantasm cost Cards are ordinary
public Discards, and the created virtual element and level are public as soon as
the ability resolves.

The Virtual Formation Card is public immediately, belongs to no Card zone, and
has no Card Instance or Card Origin. Card Interpretation Layers, Pouch bonuses,
and Star Element Substitution apply only to physical Card Instances and cannot
change the virtual component. Formation matching, preview calculation, point
formulas, effect resolution, and rules that count Formation components consume
one resolved `FormationComposition`; Card movement, hand size, covering, and
effects requiring a Card Instance consume only its physical Cards.

Illusion creates a `FormationRequirement` whose only allowed completion is the
corresponding Base Ruleset five-element strike. Phantasm creates the same kind
of requirement but permits any Base Ruleset Formation; a one-component
five-element strike means every legal element-and-level declaration has a
completion path. The server automatically adds the virtual component only for
Formation queries and submissions. The Player's selected Card set remains
physical, so non-action-ending Spirit Skills and other abilities do not treat
the virtual component as selected.

Before activation, a playable-action query exposes one Illusion or Phantasm
offer with a typed Action Input Requirement containing the legal Virtual
Formation Card elements and levels. It does not enumerate one complete offer
per element-and-level pair. Selecting those inputs remains pre-commit browser
state; only an accepted `ActivateProfessionAbility` command creates the Virtual
Formation Card, consumes the activation allowance, and establishes the
Formation Requirement. This pre-commit input is not a Pending Choice and does
not survive reconnect.

Dark Spirit creates both a physical level interpretation and a Formation
Requirement for its selected Card. It may declare level one or two only when
that value lowers the Card's current effective level, and the server
automatically includes the required Card once in Formation queries and
submissions. Pass, Profession Change, and Formations that cannot satisfy an
active Formation Requirement are illegal. Non-action-ending effects may occur
first only while they preserve a completion path. An invalid submission leaves
the requirement active; an accepted allowed Formation fulfills it even when
the Formation is later cancelled, prevented, sealed, or ineffective.

Blazing Yang Art raises its selected Wood or Fire Card by two, capped at five,
without creating a Formation Requirement. If the Player uses that Card during
the turn, its raised level is the only level available, it may participate only
in a Base Ruleset Formation, and it may not be used for Profession Change or a
Formation from another Rule Module. The Player may instead take an action that
does not use the prepared Card.

Physical Card Interpretation Layers compose by dimension and effect order.
Pouch, Fire Spirit, Blazing Yang Art, Dark Spirit, and Star effects therefore
resolve one effective element and level for each physical component. Formation
matching does not create separate printed and prepared candidates, and accepted
Formation resolution does not fall back to printed or globally cached Card
facts. The canonical Formation Requirement completion records the full
Formation Composition so replay and audit do not infer virtual facts from an
earlier activation event.

The Web does not fabricate a Card presentation, status badge, or Formation-name
annotation for a Virtual Formation Card. The public event feed records its
creation and requirement completion. When Phantasm covers a Passive Spell, the
Formation identity and physical Cards retain the existing Covered Passive
redaction, while the already-public virtual facts remain public.

Formation matching distinguishes physical Card Instances from match slots.
Sacred Art may expand one physical Card Instance into two slots for Formation
matching, but Card movement and ordinary level-sum point formulas count that
Card Instance only once. A `FormationMatchOption` records both slots as
originating from the same Card Instance. The Web presents this only as a `×2`
match annotation and keeps discard count and point previews based on the
physical Card.

Sacred Art multiplicity is applied after resolving physical Card Interpretation
Layers. It is not another element or level transformation, and it remains
incompatible with Star Element Substitution. Its duplicate match slots share
one physical Card's effective facts, while Card movement and ordinary physical
Card counts still include that Card Instance only once.

The normal Web interaction remains Card-selection first. The existing playable
Formation query deepens into a playable-Action query that returns both
Formation candidates and Profession Change candidates matching the exact
selected Card Instances; it does not enumerate every possible hand combination
or choose Cards for the Player. A separate teaching-mode projection may show
the full Profession progression graph, requirements, and ability changes.

The battlefield's current Formation control area is divided vertically at every
viewport size. The upper **Ability** panel contains Active-Effect Commands and
states that they do not enter `Action`. The lower **Action** panel contains
Formation Uses, Profession Changes, and Pass Action, and states that choosing
one enters `Action`. Do not add a separate Profession panel. Automatic
Profession Abilities and Formation Proficiencies appear through legal options
and their explanations rather than as controls.

Each Player seat with a current Profession shows a public Profession badge.
Selecting the badge opens a read-only summary of that Player's effective
abilities; activation controls remain in the central Ability panel. A Player
without a Profession shows no badge.

Deploying Hero Schools does not rewrite existing room module configuration.
Existing waiting rooms and active games lack the Hero Schools module ID and
remain Hero-disabled and replay-compatible. Newly created rooms enable the
module by default. A waiting-room owner may enable it explicitly, which uses the
existing Rule Module change behavior to invalidate readiness and locked setup
inputs.

Hero Schools may be implemented internally in vertical slices: shared
Profession foundations; one slice for each of the five Schools; Unaffiliated
Professions plus Void Reversion Technique; then cross-module, Web, teaching,
and E2E integration. The incomplete module is not added to the available
official catalog. Only the complete set of 18 Professions and end-to-end
behavior ships behind the single Hero Schools room toggle.

Validation combines exhaustive rule cases with bounded integration coverage:

- Rust conformance tests cover every Profession transition, inherited ability,
  Automatic Profession Ability, Formation Proficiency, Activated Profession
  Ability, and Profession Formation.
- Pairwise integration tests combine Hero Schools separately with Star, Five
  Directions Legend, Discard Retrieval, and Personal Deck.
- At least one two-Player and one four-Player browser flow run with every
  available module enabled.
- Base-only and Hero-disabled suites remain regression gates.

Do not require the complete Cartesian product of all Rule Module
configurations.

Hero Schools conformance is pinned to the official 5.16 rules retrieved on
2026-07-01. The linked official pages remain source references, but changes to
their mutable `latest` content do not alter this implementation's acceptance
criteria mid-delivery. A later official revision requires separate rule-upgrade
work; this change does not introduce a general Rule Module versioning system.

### 38. Official 5.16 Advanced-Rule Source Reconciliation

On 2026-07-01, all three official Advanced Rule Web pages and all six Hero
School subpages were compared with printed pages 13-24 of the
[official 5.16 complete rulebook](https://www.dropbox.com/scl/fi/4cigz3t61p07l5tkgvjyl/5.16.pdf?dl=0&rlkey=m2zx1x5dwb6cw7llyy8wt9xiy&st=mqligrpj).
The Web pages remain the normal readable source. The following PDF-only details
are normative supplements because they change matching, timing, or resolution;
differences that merely restate a rule or add a non-distinguishing example are
not listed.

#### Star Chart

Compared with the
[Star Chart Web page](https://www.cfecards.org/rule/latest/star), printed page
13 adds:

- A qualifying Star summon is mandatory unless that Star already exists or
  another rule makes the summon impossible.
- Star element substitution changes a Card only while testing Formation
  composition. At every other time the Card retains its printed element.
- When element substitution is used to cover a Passive Spell, the Player must
  declare the substitution and identify the affected face-down Card when
  covering it. The Card is not revealed.

The Web page already states that substitution affects only one Card and only
Base Rule Formations, so it cannot be used for Advanced or Theme Formations or
Profession Changes.

#### Five Directions Legend

Compared with the
[Five Directions Legend Web page](https://www.cfecards.org/rule/latest/field),
printed page 15 adds:

- A Sacred Beast changes the Environment after resolving its damage, so its
  own damage is modified by the previous Environment.
- Immunity to Formation effects does not imply immunity to non-Formation
  rules. Profession resistance still applies to Sacred Beast damage.
- Environment damage doubling occurs before five-element generation,
  overcoming, or neutralization.
- When Environment conversion and five-element generation would both turn the
  same damage into recovery, recovery occurs once rather than twice.
- Void Meridian Severing deducts HP only if it actually clears an Environment.
  No HP is deducted when there is no Environment or another effect prevents
  the clear.

#### Hero Schools: General Rules

Compared with the
[Hero Schools Web page](https://www.cfecards.org/rule/latest/hero) and its
school subpages, printed pages 17-18 add:

- Profession Change resolves in this order: declare the Profession, reveal the
  required Cards, reveal and resolve the Previous Player's covered Passive,
  change Profession, then discard the required Cards.
- Profession Change is an Action but is not a Formation. Effects that apply to
  Formation Uses do not apply to it.
- Automatic Profession Abilities cannot be declined. An Activated Profession
  Ability cannot be used while the Player Cannot Act or when its complete
  effect cannot be achieved.
- Unaffiliated Professions are not one School. Even two Players with the same
  Unaffiliated Profession do not count as sharing a School.
- Elemental resistance makes the damage invalid. Consequently, five-element
  generation from that damage does not recover HP.

The Warrior Web page already contains the PDF's Shield clarification:
resistance and Unloading apply only when the Player receives damage, so a
Shield receives the unreduced damage.

#### Hero Schools: School-Specific Rules

- **Warrior:** printed page 19 and the
  [Warrior Web page](https://www.cfecards.org/rule/latest/hero/warrior)
  conflict on how physical Card placement distinguishes Defense from
  Countershock when Wood and Fire are covered together. The PDF prefers the
  left Card when they do not overlap and uses the Card nearer the Player only
  when left/right is unavailable; the Web page prefers the nearer Card first
  and uses the Player's left only when near/far is unavailable. The online
  engine does not infer physical layout. It requires the covering Player to
  select Defense or Countershock when both are legal and records that
  selection.
- **Seeker:** printed page 20 adds one distinguishing Reincarnation example
  absent from the
  [Seeker Web page](https://www.cfecards.org/rule/latest/hero/seeker). With
  Wood 2, Wood 3, Fire 3, and Earth 4, only Wood 2 may be the standalone Wood
  Card because Wood 2 + Fire 3 + Earth 4 totals only 9. There is one legal
  option recovering 50 HP. A choice is required only when multiple role
  assignments are legal.
- **Mesmer:** printed page 21 defines the result of Illusion and Phantasm as a
  `虛擬牌` created after discarding two hand Cards, not as a third physical hand
  Card receiving a new interpretation. It also explicitly states that Phantasm
  and Illusion are different Activated Abilities, so Phantasm does not trigger
  Illusion Refinement (`幻術精研`). This is consistent with, but not stated
  directly by, the
  [Mesmer Web page](https://www.cfecards.org/rule/latest/hero/mesmer).
- **Windwalker:** printed page 23 classifies Instant Shadow Death's halving as
  an HP deduction. Resolve it by setting the target Team's HP to
  `floor(original HP / 2)` and record the resulting negative HP delta. The
  [Windwalker Web page](https://www.cfecards.org/rule/latest/hero/windwalker)
  states the round-down rule but omits the HP-deduction classification.
- **Mage and Unaffiliated:** no additional semantic rule remains after
  combining the general Hero Schools page with the respective
  [Mage](https://www.cfecards.org/rule/latest/hero/mage) and
  [Unaffiliated](https://www.cfecards.org/rule/latest/hero/others)
  subpages.

### 39. Theme Rule Module Delivery Scope And Defaults

The global-catalog default policy below is superseded by
[ADR-0038](adr/0038-bind-games-to-rule-versions.md): available themes come from
the selected Rule Version, independently of the game's enabled module subset.
The remaining execution decisions in this section continue to apply.

Add Jianghu, Confluence Generation, and Dark Glimmer as three independently
selectable Theme Rule Modules, delivered in that order. Each delivery is a
complete vertical slice through the Rust rules engine, canonical replay,
viewer-filtered projections, Online Game Room configuration, UI, and automated
tests.

The normative source is the supplied `cfecards-5.16.pdf`. Implement the complete
rules for those three Theme Rule Modules and their interactions with the Base
Ruleset, all three Advanced Rule Modules, and the Spirit Rule Module. References
to unimplemented Theme Rule Modules, such as Delayed Spells, remain inert until
those modules are separately added; do not invent placeholder behavior.

Every Theme Rule Module is independently selectable and may be combined with
the others. Like Spirit, Jianghu and Confluence Generation require Star, Five
Directions Legend, and Hero Schools. Dark Glimmer additionally requires Spirit
because its Evil and Death Spirits use the Spirit rules. Selecting a Theme Rule
Module automatically selects its transitive dependencies; removing a dependency
removes every Theme Rule Module that requires it.

New games and Online Game Rooms enable every available Rule Module by default,
including Spirit, Jianghu, Confluence Generation, and Dark Glimmer. Persisted
rooms retain their explicitly stored Rule Module list rather than gaining newly
released modules implicitly.

Pouch follows the same default after its release despite adding Initial Pouch
Selection to Game Preparation. Existing rooms retain their stored module list;
only newly created rooms receive Pouch by default.

#### Pouch execution model

Sheep Stealing carries both Card selections in one command, but the two sets
do not share one validation snapshot. Validate the first set against the
pre-effect Personal Deck, project those ordered Deck-to-Discard moves, and only
then validate the return set against the performing Player's projected Discard
Pile. This permits the same physical Card Instance to move from Deck to Discard
and back to Deck during one resolution only for a Card with that Player's
origin; an Exposed Foreign Card returns to its origin owner's pile and is not a
return candidate. The canonical `CardsMoved` event records those deltas in
resolution
order and replay applies them sequentially.

Private interaction options expose the current Discard Pile together with
Personal Deck Cards that would enter that Player's Discard Pile when selected
for the first step. The UI presents a prospective Deck Card as a return choice
only after the Player selects it for discard. This is an interaction projection
of the ordered effect, not a new canonical Pending Choice.

Chain's ownerless `PouchRevealed` event sets the triggering source Card aside
from its Player's Deck. The selected Pouch Card is removed by `PouchPlaced` in
the same event batch. Chain then projects the selected Secret Strategy before
requesting its post-search Deck Shuffle; the later `PouchConsumed` event moves
the set-aside source Card to its origin Discard Pile. The shuffle's recorded
`currentOrder` therefore excludes both selected Cards and still matches the
Deck when trusted randomness is resolved. Sheep Stealing is the exception to
the additional Chain request: its own post-exchange Deck Shuffle is reused, so
the Chain flow does not issue a duplicate shuffle.

離山 deliberately replaces official 5.16's ineffective-result interpretation
with **Temporary Ability Loss**. Official rule 4-7's broad
「該職業之能力」boundary is authoritative: the affected Player retains their
Profession identity but temporarily lacks its complete Profession Ability Set,
including Automatic, Formation Proficiency, and Activated Profession Abilities,
Profession Formations, inherited rules, and other Profession rules. The Spirit
scope likewise removes active, automatic, and persistent Spirit Skills and
continues to prevent Spirit Power gain. See
[ADR-0033](adr/0033-treat-lure-as-temporary-ability-loss.md).

Playable-action generation and Command validation use the same current-ability
authority. An option requiring a lost Profession Ability Set or Spirit Skill is
not offered, and a stale or direct submission is a Validation Failure that
emits no canonical events or costs. Ordinary Profession Change remains legal
when its path is independent of the lost set: the 抉擇 and 突破 permissions
granted by 初行客 are absent, the retained Profession identity may still satisfy
an ordinary prerequisite, and the absent 暗行 prohibition no longer blocks an
ordinary Profession Change. Profession Formations and profession-granted
transitions or acquisition consequences remain unavailable for the duration.

Temporary Ability Loss is prospective rather than retroactive. An already
accepted ability use retains its Prepared Profession Ability, Formation
Requirement, Card Interpretation Layer, bonus, Status Effect, cost, and consumed
usage; neither applying nor expiring 離山 rolls those facts back or resets them.
Automatic checks during the loss do not use the absent ability, including 綻放
and 順風回復使用次數. Profession Change, Spirit Summoning, and other
source replacement do not escape the Player-scoped duration.

The existing Profession-scope and Spirit-scope 離山 Status Effects remain the
canonical and Public State representation; no event or Pending Choice shape
changes. 金蟬 continues to protect only the Player-facing Profession scope, not
Spirit Skills or Spirit Power gain. Public presentation retains the owned
Profession, Spirit, and read-only summaries, relies on the existing visible 離山
Status, and derives actionable controls only from Playable Actions. Pure replay
continues to apply older canonical events exactly, including an ability use that
was accepted under the previous interpretation; live decision logic carries no
compatibility branch for submitting that action now.

The Jianghu term **State (狀態)** is narrower than the engine's established
generic `StatusEffect` concept. Define a separate typed Jianghu State collection
containing only 千鋒, 踏雪, and 中毒; do not rename or change the generic model or
serialized `statuses` contract. New canonical and Public State fields use
backward-compatible defaults. See ADR 0012.

Remove the existing Profession teaching/catalog dialog rather than expanding it
for the new Theme Professions. The battlefield retains each Player's current
Profession badge and read-only effective-ability summary. Public Limited Use
counts from Confluence Generation appear with the granting Profession or
ability because they are required current game state, not teaching content.

All enabled Rule Modules contribute to one composed Profession catalog and one
Player-owned Profession slot. Module-owned definitions and typed hooks remain
separate even when a Profession explicitly inherits abilities from a Profession
defined by another module. See ADR 0013.

Jianghu poison has one structurally unique source: every poison-producing
Formation affects its performing Player's Next Player, and fixed Turn Order
gives each poisoned Player exactly one Previous Player. Repeated poison
applications therefore add to one remaining-turn count; do not introduce
per-source poison segments. At each poisoned Turn End, Poison Mastery reads the
unique source Player's then-current Profession, which also covers poison applied
before that Player changed into 毒聖.

The source terms `星行牌` and `環行牌` both read printed Card elements rather
than temporary Card interpretations. A Star-Element Card matches the Star
currently owned by the relevant Player's Team; an Environment-Element Card
matches the Environment current at the rule's check timing. If the corresponding
Star or Environment does not exist, no Card qualifies.

Confluence Generation's `上家棄牌` is the existing Retrievable Discard: the
Previous Player's Turn Draw Discarded Card from the immediately completed
Previous Turn, provided that same Card Instance remains in any Discard Pile;
the current Pile Owner does not affect retrieval. Moving it to a Deck, hand, or
any other non-Discard zone makes the physical Card unavailable. Residual
Element (`餘行`) and Residual Level (`餘級`) are instead fixed from that Card's
printed definition when it is Discarded. They remain unchanged until the
current Player reaches Turn Draw even if the Card moves; moving it only makes
調律 or 天響 unable to retrieve that physical Card.

Dark Glimmer's Mischief (`戲鬧`) calculates its HP deduction from the highest
printed level among the Card Instances actually inspected by the triggering
effect. Evil Gaze therefore uses only its two randomly inspected Cards, while an
effect that inspects a complete hand uses that complete hand. Canonical events
record the inspected set, but viewer filtering reveals it only to the inspecting
Player and never supplements it with uninspected hidden Cards.

晴風使的順風只在 Discard Shuffle 完成時回復使用次數；reordering Cards already
in a Deck never recovers it. A shared-Deck Discard Shuffle recovers the use count
of 順風 for every
Player who currently owns that ability and whose Profession Ability Set is
available. With Personal Deck enabled, only the owner of the shuffled Discard
Pile can recover it. Recovery records an explicit `LimitedUseChanged` only for
an actual increase from zero to the current maximum; a Player already at
maximum, whose Profession Ability Set is unavailable, or who no longer owns
順風 receives no recovery event.

Local room `44e4b60f-89c9-4e7e-a832-521af4a5aa3c` is the regression scenario
that fixed this distinction: command 39 consumes player-2's 順風, and
command 72 performs Rusted Iron Withered Forest against player-2's Personal
Deck. Its trusted shuffle is a Deck Shuffle, so 順風 must remain exhausted.

Death Spirit's Shared Fate (`同命`) is **not** derived from the resolved
Formation's Affected Player Set. The confirmed rule is: when a Formation effect
actually deducts the Death Spirit owner's Team HP, that owner's next Player's
Team loses 10 HP. The effective Team HP delta is therefore the relevant fact in
this Team-HP model. An `AttackResolved` event's base Attack damage does not
qualify. A Formation's separate HP-deduction effect does qualify after immunity
or prevention has made its effective delta known, including when atomic event
serialization places that separate effect beside its Attack result. Shared Fate
is appended after that qualifying deduction, groups multiple
owners that share a target Team into its single resulting HP change, and cannot
recursively trigger another Shared Fate.

Under Void Spirit-Shattering, a Death Spirit that remains owned after losing two
Spirit Power triggers Shared Fate only when that Technique's Formation effect
deducts its owner's Team HP. A Death Spirit reduced to zero and broken by that
Technique does not trigger, and a Spirit newly summoned by 魔靈復甦 did not exist
for the triggering deduction and does not trigger retroactively.

Death Omen (`死兆`) uses the existing trusted Discard Shuffle boundary when the
next Player's applicable Deck has one to three Cards and its Discard Pile is
non-empty. Its `RandomnessRequested` contains a typed Spirit `deathOmen`
continuation and is the only initial canonical fact: it does not consume the
Skill's once-per-turn allowance or Spirit Power, move Cards, change HP, or
break the Spirit. After `RandomnessResolved` has moved the shuffled complete
Discard Pile to the bottom of that Deck, the continuation emits the ordinary
Skill-use and resolution facts. Thus replay, the Online Room canonical record,
and Public Views observe the same waiting state and recorded shuffled order.

#### Jianghu execution model

Canonical Game State stores Jianghu States separately from generic Status
Effects. 千鋒 and 踏雪 retain explicit expiry through the end of the owner's next
turn and apply immediately to the Formation that created them. 中毒 stores one
aggregate remaining-turn count on its affected Player, deducts HP and decrements
at that Player's Turn End, and blocks only Formation-provided HP recovery.
Applying, extending, reducing, and ending a Jianghu State is event-recorded.

Typed Jianghu hooks participate at the stages the published rules require:
Formation matching, Attack-point calculation, Formation-effect immunity,
recovery resolution, post-Formation consequences, and Turn End. They return
declarative deltas and do not mutate Game State directly.

#### Confluence Generation execution model

Limited Uses are public canonical records keyed by Player and stable Formation
or Ability identity, with explicit maximum and remaining counts. Consumption,
recovery, maximum changes, and Profession-reacquisition resets are semantic
events. Changing from 道法師 to 道法聖 increases 禁錮法陣's maximum and remaining
count by one without resetting prior use; 無盡法陣 restores it to the current
maximum.

調律 creates a turn-scoped, Card-specific use obligation: the retrieved Card
must participate in that turn's Profession Change, or in the permitted
Profession Formation while 易弦 applies. Activating 調律 is legal only when at
least one such action can be completed. 天響 retrieves the same Card without
that restriction. Choices that inspect a Deck, retain Cards, select a 五鳴術, or
decide 晴風's revealed top Card use typed Pending Choices and replayable
continuations.

The 調律 legality check is reachability-based rather than an immediate
post-ability check. It includes remaining non-action-ending abilities and
allows only intermediate choices whose projected result preserves at least one
legal completion path. The Rules Engine applies this rule to both action
queries and command validation. The obligation is fulfilled when the 調律 Card
participates in an accepted allowed action even if that action's effect is
later ineffective or cancelled; no other voluntary effect may consume that
Card.

The 調律師 system's Residual-Element transition totals 3, 6, and 9 are minimum
totals. Residual Level is a printed baseline, while submitted Cards and 調律's
Discard cost compare their effective levels after applicable interpretations.
Each Formation role requires a distinct physical Card unless a rule explicitly
grants multiplicity, so a five-resonance Formation's element-Card and
Residual-Level-Card roles cannot be filled by the same Card Instance. 易弦's
permission covers every currently available Profession Formation, including
inherited 調律師 Formations.

千鳴 normally resolves both resonances simultaneously. When 鏡鳴 requires a
choice, fully resolve the Residual-Element resonance continuation before the
selected resonance; a lethal earlier resonance does not truncate the later
one, and Game Outcome waits for the complete Formation. 萬鳴 instead applies
its deterministic HP, Shield, and Turn Draw changes before entering its 鏡鳴
choice, while still deferring final Game Outcome until that choice completes.

煌鳴 and composite resonances containing it affect only the Previous Player;
their teammate shares the Team HP change but is not added to the Affected
Player Set. 森鳴 and 萬鳴 recover the performing Player's Team HP, but 中毒 or
other performer-scoped recovery restrictions are checked only on that
performing Player, not their teammates.

#### Dark Glimmer execution model

惡精靈 and 死精靈 extend the Player's existing single Spirit slot and retain the
zero-through-six Spirit Power rules. 魔靈附體 transforms the current Spirit kind
without changing power; 魔靈復甦 and the two summoning Formations summon a new
Spirit at the published initial power. Persistent Spirit Skills execute through
typed hooks and never consume the once-per-Spirit-per-turn activated Skill
allowance.

暗行 is one shared ordinary-Profession-Change permission predicate. Both
playable-action generation and command validation use it, so 暗行者 and its
inheriting 暗靈使 offer and accept no ordinary Profession Change. The automatic
暗行者 → 暗靈使 transition is a Dark Formation consequence and remains outside
that ordinary command predicate.

Random hand selection and inspection use the trusted application randomness
boundary; canonical events record the selected Card Instances and their order,
while Public Views preserve the rule's inspection boundary. Dark Formation
Environment invalidation is keyed by performed Formation identity. 暗境 bypasses
that invalidation only for its owner, and the Hero Schools 影遁 hook ignores only
the two additionally named Dark Formation effects.

### 40. Tribulation Theme Rule Module Scope And Source Interpretations

Add Tribulation as an independently selectable Theme Rule Module and deliver it
as a complete vertical slice through the Rust rules engine, canonical replay,
viewer-filtered projections, Online Game Room configuration, UI, and automated
tests. It follows the existing Theme Rule Module dependency and default-selection
policies.

Each of the five Tribulations is a variable-card-count Formation: it contains
exactly its two specified overcoming elements, with one or more Cards of each
element and an effective level sum of at least seven per element. In practice
the five-Card hand limit permits four or five physical Cards. Under official rule
2-5.5c, a Tribulation never satisfies a rule that requires a fixed Formation
card count, regardless of how many Cards were physically used.

For Earth-Rending Mountain Collapse (`裂地崩山`), the source phrase `環行牌`
means a Card whose printed element matches the Environment after the Formation
transfers it. The new Environment therefore determines every Player's eligible
discard.

The performing Player may select any of the five Environments, including the
currently active one. A same-element selection still emits an Environment
Transfer and still requires the Environment-matching discards.

Official rule 5-2.4i makes an Attack's damage and additional effect simultaneous.
Earth-Rending Mountain Collapse therefore records the declared Environment and
collects Player choices sequentially from the Next Player without applying
Formation consequences between answers. After the last answer, one atomic
`AttackResolved` transfers the Environment, applies every discard or hand
reveal, resolves the 60-point Attack, and only then evaluates Game Outcome. Choice
request and answer events remain replayable intermediate facts, not early
application of the Formation effect.

For Rusted Iron Withered Forest (`鏽鐵枯林`), a shared Deck is processed once:
reveal its top eight Cards, discard Cards of level three or higher, then shuffle
the rest back into that shared Deck. With Personal Deck enabled, each Player's
Deck is processed separately. Both paths reorder Cards already in a Deck and
therefore never recover the use count of 順風.

In team play, Divine Calculation Status makes Thunder-Fire Tribulation's
(`天雷劫火`) 15-point global HP deduction ineffective for the Status owner's
Team. Team HP is the Player's HP-bearing resource, so applying the immunity only
to an individual Player would make this part of Divine Calculation ineffective
in team play.

Divine Calculation's 20-point reduction applies only when its owner personally
receives Tribulation attack damage. If the owner's Shield takes the damage
instead, the Shield takes the unreduced amount because the Player did not receive
that damage. The Status still makes that Tribulation's additional effect
ineffective for its owner and is still removed after the Tribulation.

Divine Calculation Status owns its protection after the 神算 Formation grants
it; applying that protection does not repeat or resume the granting Formation's
effect. Snow-Treading Status therefore does not bypass Divine Calculation's
20-point reduction, although Snow-Treading still handles direct Formation
effects against the Attack through its normal rules.

Divine Calculation does not prevent Rusted Iron Withered Forest from processing
a shared Deck because that Deck is not owned by the protected Player. With
Personal Deck enabled, skip only the Status owner's Deck and process every other
Player's Deck normally.

Against Earth-Rending Mountain Collapse, Divine Calculation does not prevent
the shared Environment Transfer. Its owner neither discards an
Environment-Element Card nor reveals their hand; every other Player resolves
that part of the effect normally.

Mudslide Torrent (`泥石轟流`) increases from 60 to 80 Attack Points only when
its additional effect effectively deducts at least one point from any Shield.
Shield loss caused later by the Attack itself cannot trigger the increase. A
Shield protected by Divine Calculation contributes no effective deduction to
this test.

Gale-Rain Status (`烈風暴雨狀態`) invalidates life recovery based on the
Formation's performing Player, matching the Jianghu poison precedent. A
Formation performed by a Player without Gale-Rain Status may still recover that
Player's Team HP even when a teammate has the Status. Non-Formation recovery
remains effective.

Official rule 6-1 governs each Gale-Rain Status duration independently. The
performing Player's current Turn End counts as that Player's first affected
turn; every other Player counts their next two Turn Ends. Repeated applications
overlap as distinct two-turn effects rather than merging or extending one
counter. Divine Calculation prevents only the new application from the
Tribulation it answers and does not remove an older Gale-Rain Status.

Earth-Rending Mountain Collapse makes a Player **reveal** their hand when no
Environment-Element Card exists; it does not let another Player **inspect** that
hand and therefore does not trigger Evil Spirit's Mischief. Thunder-Fire
Tribulation's `both Teams` HP deduction is a Formation effect: Shared Fate
triggers for each surviving Death Spirit owner whose Team actually loses HP.
A Team protected by Divine Calculation has no such HP deduction or Shared Fate
trigger.

Playable action detail is an optional, contextual player-facing decision
contract rather than Formation Catalog `rule_text` or a standalone restatement
of the offered Action. For Echo Melodies it supplements the visible offer with
the relevant Echo policy: the five optional-cost Melodies state the allowed
Echo Cost elements and their conditional next-Turn-Start repetition,
變徵‧淨火 states its automatic no-cost repetition, and 變宮‧植土 states its
distinct next-Turn-Start Melody-main-effect choice. Positive wording such as
repeating the main effect once expresses the player-visible boundary without
describing whether the engine creates another Formation Use.
[ADR-0023](adr/0023-separate-action-detail-from-catalog-rule-text.md) owns the
typed rule-fact boundary, refined by
[ADR-0028](adr/0028-treat-action-detail-as-contextual-supplement.md), which
defines contextual completeness.

`SpiritLevelInterpreted.skill` is an optional canonical semantic field. New
records write the Fire Spirit Skill that created the Card Interpretation;
records created before this field existed deserialize it as absent. Replay
preserves those records and the Public View presents the absent value as a
legacy Fire Skill interpretation instead of guessing between 螢光 and 絢爛.

### 41. Independent Initial Pouch Selection And Hard Cutover

Initial Pouch Selection remains a public, reconnectable Game Preparation stage
owned by the Rules Engine, but it no longer names one expected Player. Every
Player who has not yet placed an initial Pouch may submit `ChooseInitialPouch`
immediately. Each accepted command commits its canonical events independently
in server arrival order. Do not represent the group as one Pending Choice or
one open multi-Player resolution Transaction.

Derive the Players who have not chosen from Turn Order minus the owners of
already placed initial Pouches. Do not persist a second `remaining_players`
collection in canonical Game State. Validate that the actor is in Turn Order,
has not already chosen, and selected a Card Instance in their own current
Personal Deck. A second command with a new Command ID returns the specific
`InitialPouchAlreadyChosen` Validation Failure and emits no events. A retry with
the same Command ID remains governed by the Online Command Transaction receipt.

Each accepted choice emits `PouchPlaced` followed by
`InitialPouchChosen { player, card }`; remove `next_player` from the latter.
The last outstanding choice additionally emits
`InitialPouchSelectionCompleted` before the first `RandomnessRequested`. Only
that completion event advances Game Preparation to `PendingDeckShuffle`.
Shuffle the remaining Personal Decks and perform the initial deal in Turn Order
exactly as before. This policy applies to both supported capacities, two and
four Players.

The choice command batches commute: canonical storage still serializes commits
and validates `expectedSequence`, while the final Rules Engine projection is
independent of accepted choice order because every choice changes a distinct
Player-owned Deck and Pouch. No canonical rollback or order-normalization layer
is required. The event log deliberately retains arrival order for audit and
replay, and replay must project the same final state for every legal order.

Public State exposes the stage as the exact camelCase contract
`initialPouchSelection: { remainingPlayers: PlayerId[] } | null`. Completed
Players are derived from Turn Order minus `remainingPlayers`; Card identity is
never added to this public progress object. The viewer-specific interaction
allows choosing exactly when that viewer remains outstanding. The Pouch Owner
may inspect their chosen Card after submission and reconnect, while every other
viewer sees a Card Back. The Web interaction keeps immediate Card-click and
random-shortcut submission without a confirmation step.

Online active-game responses carry an active-game version consisting of
`gameInstanceId` and `recordSequence`. For the same Game Instance, the browser
ignores a response whose sequence is lower than the highest response already
applied. This is a delivery-order guard for HTTP and WebSocket snapshots, not a
canonical concurrency mechanism. Do not add a global room revision as part of
this change.

This release is a hard cutover. It does not read, migrate, repair, or preserve
active legacy Game Records, and it does not preserve completed legacy Replays.
Waiting rooms remain available with their room identity, owner, members, seats,
invitation, enabled Rule Modules, readiness, and Locked Deck Lists. Active and
Finished rooms return to Waiting, clear `gameInstanceId`, and mark every member
unready and disconnected while preserving the room configuration. Legacy game
records, replay drafts, locked Deck Lists belonging to reset games, canonical
room events, Command and trusted-randomness receipts, and resolution
transactions are removed. Each surviving room starts a fresh room-event log
with `LegacyGamePurged { epoch }`, presented once as
`系統版本更新，上一局已清除，請重新準備。` Legacy replay archives and their
D1 references are deleted permanently; the existing ReplayArchive class and
namespace continue storing Replays created by the new version.

Perform the cutover through a protected, one-time management CLI rather than a
D1 or Durable Object migration. The CLI requires an explicit staging or
production environment, defaults to dry-run, requires a separate confirmation
flag for mutation, reads credentials only from the environment, supports a
stable purge epoch, and is safe to rerun. It uses a protected GameRoom purge
operation for partial room cleanup, a ReplayArchive purge operation backed by
Durable Object `storage.deleteAll()`, D1 cleanup of only replay-reference and
lifecycle rows, and the Cloudflare Durable Objects Objects API to find stored
objects that D1 references may not cover. The application remains in explicit
maintenance mode throughout mutation and verification; command submission and
Replay creation are unavailable, and any partial failure leaves maintenance
enabled for a safe retry. Reopen traffic only after verifying that legacy Game
Records are absent, replay D1 tables are empty, replay Durable Object storage is
empty, old Replay IDs return not found, and preserved rooms still exist.

### 42. Deep Secret Strategy Decisions And Hard Cutover

Deepen Secret Strategy resolution as an internal child module of the Rust Pouch
Rule Module. It owns Secret Strategy offer construction, complete Decision
validation, immediate strategy effect events, and strategy-specific
continuations. The outer Pouch module retains source acquisition, Pouch reveal,
placement and consumption, the direct-versus-Chain lifecycle, and Chain's
post-search shuffle. Keep the external `OfficialRules` interface unchanged and
do not create a public plug-in or per-strategy extension seam.

Replace the current wide optional-field command bag with one closed algebraic
Command/Parameter Object. A `SecretStrategyDecision` retains its source Card and
one answer-shaped selection from exactly five families:

- a no-input Secret Strategy, covering 金蟬, 偷梁, 混水, 觀火, 還魂, and 暗渡;
- 離山 with one target Player;
- 瞞天 with `Gain(star)` or `Break(star)`;
- 走為 with `Clear` or `TransferByDiscard(card)`;
- beginning 牽羊, with no Deck or Discard Pile Card selections.

The closed variants must make irrelevant fields and invalid field combinations
unrepresentable. In particular, replace `star + breakStar` with the Star
operation and replace an optional discard Card with the Environment operation.
Use exhaustive typed Rust dispatch rather than ten shallow Strategy objects.
Direct Pouch triggering carries the complete Decision. Chain's answer is a
separate closed outer envelope: `PlaceOnly`, or `PlaceAndTrigger` containing the
same complete Decision. The two paths share Secret Strategy semantics without
merging their outer lifecycles.

Secret Strategy offers follow the same answer-shaped families. Each option
contains only the candidates relevant to its answer: no-input strategies have
no empty candidate collections, 離山 exposes target Players, 瞞天 exposes legal
Star operations, 走為 exposes legal Environment operations, and 牽羊 exposes
only the fact that its deferred exchange can begin. Remove generic
`requiredCardCount` and unrelated empty option fields. These private
interaction projections may cross the Rust-to-TypeScript boundary, but the
Decision is not canonical Game State and is not added to Public State or the
Public Event Feed.

Submission revalidates the complete Decision against current canonical state;
there is no opaque Option ID and no trust in an earlier offer snapshot. Validate
the source Card, selected strategy eligibility from its printed value, current
actor and target authority, and every typed operation before emitting any
event. A stale, forged, mismatched, or otherwise illegal Decision is a
Validation Failure with zero canonical events and no reveal, consume, cost, or
movement. A Decision created internally after successful validation that the
module cannot resolve is instead a Rule Implementation Error.

瞞天 `Break(star)` requires that Star to exist when submitted. A nonexistent or
stale target is a Validation Failure rather than a legal no-effect resolution.
This differs from a granted Temporary Star Effect whose Formation later tries
to break a same-named Star its Team does not own; that later break remains a
legal no-effect consequence. 走為 `Clear` is legal without a current
Environment and resolves as no change. `TransferByDiscard(card)` requires a
current hand Card Instance, reads its printed element, permits an Exposed
Foreign Card, returns the Card to its Card Origin Discard Pile, and remains
legal with no current Environment or with a same-element current Environment.

牽羊's initial Decision only begins its continuation. After any required
preliminary Discard Shuffle, retain the later typed Sheep Stealing Pending
Choice: select exactly two current Deck Cards, project those ordered moves into
the Discard Pile, then select exactly two Cards from that projected Discard
Pile. The existing sequential validation allows either newly discarded Card to
return. The Secret Strategy source Card remains set aside and ineligible until
the exchange and trusted Deck Shuffle complete.

All immediately decidable validation precedes canonical output. A direct valid
resolution orders `PouchRevealed`, the strategy effect, then `PouchConsumed`.
Chain orders `ChoiceMade`, `PouchPlaced`, `PouchRevealed`, the strategy effect,
then `PouchConsumed`, followed by its ordinary post-search shuffle when
required. A deferred 牽羊 resolution reveals first and consumes only after its
typed continuation and shuffle finish; its shuffle continues to replace
Chain's otherwise additional post-search shuffle.

TypeScript is only the browser-local adapter from a draft to the closed command
DTO. It must not reproduce strategy eligibility or Rules Engine validation.
Rust-to-TypeScript tagged variants and every new multiword field use exact
camelCase serialization contracts with explicit tests.

This is an atomic hard cutover. Remove the legacy optional-field command and
option shapes in the same change, without dual reads, compatibility variants,
or fallback parsing. Preserve all unique behavioral evidence. Before deleting
or consolidating existing Pouch tests, inventory every assertion claim and name
its replacement evidence as required by ADR-0032. Keep the interaction matrices
and representative Online Game Room and Playwright seams while adding focused
atomic-validation, canonical-order, replay, and serialization coverage.

### 43. Battle Record Turn-Relative Narration

Project the viewer-filtered Public Decision Feed into a deterministic Battle
Record for both live rooms and Replays. A `TurnStarted` fact immediately creates
its labeled Turn Group, including when that group has no Entries; it is a
boundary, not a separate Battle Record Entry. Preparation remains a distinct
pre-Turn group and may retain its existing empty-group display policy.
Room and Replay views display that empty Turn Group without a placeholder
Entry; a new group is new visible content for automatic scrolling.

Within a Turn Group, omit the subject only where the typed semantic subject is
the Turn Player. Do not derive this by globally deleting rendered names. The
narrator instead chooses the sentence at each typed Player role boundary, so
another decision maker and every Player needed as a target, owner, source, Team
reference, or next decision maker remains explicit and keeps normal
viewer-relative labels (`你`, display name, `我方`/`對方`, or observer labels).
Preparation has no Turn Player and therefore omits no Player subject.

Each accepted Player Decision remains one Entry; a later Decision by another
Player starts a later Entry, while its automatic consequences, passive flips,
and Pending Randomness remain with their existing Decision boundary. The Entry
title names the complete Decision using public typed facts (for example the
Formation, Spirit Skill, Profession Ability, or Secret Strategy). Its summary
only supplements public costs, selections, reasons, and consequences, and must
use the official element-card label format (`金`/`木`/`水`/`火`/`土`, a space,
then level). Do not restate the Decision already named by the title: a Formation
title can be followed by `使用火 2、水 1。`, and a named Spirit Skill title by
its Spirit Power change and other public consequences.

The Battle Record is not persisted rendered text. Old Replays project their
canonical viewer-filtered inputs through the current deterministic narration
rules, so equal viewer-filtered input and perspective always produce equal
wording. Do not add NLG, LLM, or Fluent dependencies for this projection.

Validate this behavior by extending the existing Battle Record and start-API
tests, scroll tests, and battlefield layout scenario. Cover Turn start before
the first Entry and the same Player in subject, target, and owner roles. A
dedicated TurnStarted integration test or new E2E spec is not required.

### 44. HP Change Planning

Every outer rule resolution constructs one validated Team HP ledger. The ledger
requires one current and one initial entry for each Team, rejects invalid ranges
or arithmetic overflow as an Engine Invariant, and records every requested HP
attempt including prevention, clamping, and no-op changes. Repeated changes to
the same Team inherit the preceding planned new HP in canonical call order.

One resolved Formation effect receives a scoped ledger session. Its completion
reports only the Teams that actually lost HP, in canonical `state.hp` order;
Shared Fate is planned afterward with the outer ledger and cannot recurse into
that session. Echo repeats are not Formation effects, while Jianghu States own
their continuous Formation-effect timing. Pending continuations construct a
fresh ledger from their then-canonical state rather than serializing a live
planner.

### 45. Totem Formation Rule Interpretations

Source: supplied 五行戰鬥牌5.17規則書.pdf, printed pages 38-39.
This section records accepted interview rulings; unresolved interactions remain
outside its scope until separately settled.

Use 青角圖騰 consistently. The 東靈陣‧青角 result naming 青爪圖騰 is a
typographical error, not another Totem kind.

Each of the five Totem-granting Formations requires exactly three Cards of its
element, with at least one having interpreted level exactly 5. For example,
木1、木2、木5 matches 東靈陣‧青角; 木1、木2、木6 does not. Existing legal
Card interpretations apply only within their granted scope; this ruling does
not expand base-only interpretation abilities to Theme Formations.

A Totem's first effect prevents Environment damage doubling only when its
Player receives damage, not when that Player's Shield receives it. In a Wood
Environment, a 20-point Wood Attack against a Player with 青角圖騰 still
deals 40 to their Shield, while the Totem removes that Environment doubling
when the Player has no Shield. Other applicable modifiers remain independent.

Only Void Meridian-Severing Technique successfully clearing an existing
Environment breaks all Totems. Performing it with no Environment, clearing the
Environment with 走為, and ordinary Environment Transfer preserve Totems.

南靈陣‧朱羽 first undergoes Active Spell effectiveness checks. If ineffective,
its entire effect does not execute, including its conversion into an Attack.
If effective, it becomes the selected elemental Attack and undergoes applicable
Attack-damage checks. Seal can therefore suppress the complete Formation,
while Defense can suppress its damage after conversion. This follows the
existing active-Spell-before-conversion treatment of Metamorphosis.

A Totem's third effect triggers and consumes that Totem when an applicable
Environment conversion into HP recovery is prevented. It does not trigger when
a Shield receives the Attack, when a Sacred Beast ignores the Totem, or when
the damage is already ineffective, for example through Defense. If independent
five-element generation also converts the Attack into recovery, the Totem is
still consumed for preventing the Environment conversion; the generation-based
recovery remains. Consumption is mandatory, not an optional Player choice.

南靈陣‧朱羽 completes Attack Resolution using the original Environment and
original Totem, then transfers to Fire and grants 朱羽圖騰. For example, in a
Fire Environment, selecting a Wood Attack may consume the performer's existing
青角圖騰 before they gain 朱羽圖騰. Damage prevention alone does not prevent
the later Environment Transfer or Totem acquisition; making the initial Active
Spell ineffective prevents all of them.

A Totem's second effect exempts the performed Formation identity from its
Environment invalidation, not the identity of an effect copied by Metamorphosis.
黃鱗圖騰 protects 幻化; 青角圖騰 does not protect 幻化 merely because it
copies 氣壁. Other independent ineffectiveness grounds, including Seal, remain.

The Totem Formation Rule Module requires all three Advanced Rule Modules but
does not require Personal Deck. 尋龍 searches the shared Deck in shared-Deck
play and the performing Player's own Deck in Personal Deck play.

Countershock distributes the Attack into shares before each recipient's Totem
first-effect exception to Environment doubling is evaluated. In a Fire
Environment, a 20-point Fire Attack reflected by a defender with 朱羽圖騰
deals 10 to that unshielded defender and 20 directly to the attacking side when
the attacker has no Totem. The defender's exception does not protect the
attacker; the existing direct-to-HP reflected-share rule remains unchanged.

A Totem's third effect is evaluated once for the complete Attack, not once for
each Countershock share. If 青角圖騰 prevents a Wood Attack's Fire-Environment
conversion into recovery, consume it once and keep both shares in damage mode,
unless an independent five-element generation still makes the Attack recover
HP. Consuming it while resolving one share never restores Environment recovery
for another share of the same Attack.

Formation effectiveness is determined when the Formation is performed,
including the applicable incoming Counter Effects, the Environment then present,
and any Totem exemption. For a Covered Passive, this is its covering action,
not its later flip. The second Totem effect therefore participates in that
performance-time determination; later changes to the Environment or Totem do
not retroactively invalidate or revive the already performed Formation.
For example, covering an effective Defense and then having the next Player use
走為 to transfer to Metal does not invalidate that Defense. A Defense made
ineffective by Metal when covered does not recover merely because the
Environment later changes. Flip-time applicability to the incoming action and
explicit later neutralization remain separate from this determination.

Implementation gap identified during the interview: `covered_passive::trigger`
currently calls `environment_makes_formation_ineffective` against the live
flip-time state, while `PassiveCovered` records `sealed` but no performance-time
Environment ground. Correct that behavior and preserve viewer-filtered reasons
without exposing a hidden Formation early. Cover both directions of an
Environment change between cover and flip in regression checks. The existing
Metal/Defense test holds the Environment constant and does not distinguish
these timings. This gap is a confirmed correction, not an open rule question.

尋龍's selected Card is publicly shown and then placed face down on top of the
Deck after the remaining Deck is shuffled. Its public reveal remains in the
Battle Record, but drawing it into a hand follows ordinary hidden-hand rules;
the reveal does not create permanent public visibility. Existing Exposed
Foreign Card visibility still applies independently.

中靈陣‧黃鱗 requires selecting exactly one Card after inspecting a nonempty
Next Player hand; declining is not allowed. If that hand is empty, skip the
Card choice and hand-to-hand Card movement while still transferring to Earth and granting
黃鱗圖騰. Making the complete Spell ineffective prevents every effect.
