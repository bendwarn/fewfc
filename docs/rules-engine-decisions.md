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
struct CoveredPassive {
    owner: PlayerId,
    cards: Vec<CardInstanceId>,
    covered_on_turn: u64,
    reveal_timing: PassiveRevealTiming,
}

enum PublicCardRef {
    Known(CardInstanceId),
    Hidden { count: usize },
}
```

`PassiveCovered` events store the real `CardInstanceId` values so replay remains deterministic and complete.

External views are generated per viewer:

- the owner can see their own covered cards
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

1. B submits a legal action command.
2. Before B's action resolves, the engine checks A's covered passive.
3. If present, the passive flips and attempts to affect B's action.
4. The passive is discarded whether or not its effect applies.
5. B's action continues resolving unless the passive effect changes or cancels it.

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

Resolution outcomes can be represented in events:

```rust
GameEvent::ActionResolved {
    player: PlayerId,
    action_id: ActionId,
    outcome: ActionOutcome,
}

enum ActionOutcome {
    Applied,
    Prevented { reason: PreventionReason },
    NoEffect { reason: NoEffectReason },
}
```

This preserves exactly-one-action turn semantics without treating invalid commands as historical facts.

### 10. Turn Draw Pending Choice

Turn draw requires a pending choice state because the player draws `N + 1` cards and chooses one of the newly drawn cards to discard.

Use a dedicated phase:

```rust
enum Phase {
    TurnStart,
    ActiveWindow,
    Action,
    TurnDraw,
    TurnDrawDiscardChoice,
    TurnEnd,
}

struct PendingChoice {
    player: PlayerId,
    kind: PendingChoiceKind,
}

enum PendingChoiceKind {
    TurnDrawDiscard {
        drawn_cards: Vec<CardInstanceId>,
        allowed_discards: Vec<CardInstanceId>,
    },
}
```

Flow:

1. Enter `TurnDraw`.
2. Draw `draw_count + 1`.
3. Emit `CardsDrawnForTurnDiscardChoice`.
4. Move to `TurnDrawDiscardChoice`.
5. Accept `ChooseTurnDiscard`.
6. Emit `TurnDiscardChosen`.
7. Move to `TurnEnd`.

This supports mid-draw save/load, UI waiting states, online play, and deterministic replay.

### 11. Draw Limits, Hand Limit, And Discard Recycling

Hand limit is 5.

If the player's hand is already at the hand limit, skip turn draw and do not create a pending choice.

Otherwise, turn draw uses the rule intent "draw `N + 1`, then choose 1 newly drawn card to discard", where `N = min(base_draw, available_hand_space)`.

This means a player with at least one available hand slot may temporarily hold one card above the hand limit during `TurnDrawDiscardChoice`, then returns to the hand limit after choosing one card to discard.

`allowed_discards` must contain only the cards drawn by that turn draw. Cards that were already in the player's hand before the draw are not legal choices for `ChooseTurnDiscard`.

When the deck is insufficient, shuffle the discard pile first and place the shuffled discard cards at the bottom of the deck. Then continue drawing.

Deck direction:

- `deck[0]` is the top of the deck and the next card drawn.
- `deck.last()` is the bottom of the deck.

When recycling discard:

```rust
GameEvent::DiscardRecycledIntoDeck {
    shuffled_order: Vec<CardInstanceId>,
    placement: DeckPlacement::Bottom,
}
```

Replay uses `shuffled_order` directly and does not rerun RNG.

Pending implementation detail:

- none

Cards are eligible for discard recycling based on their current zone.

- formation cards that already moved to `discard` before `TurnDraw` may be recycled during that same turn's draw
- cards currently being drawn or awaiting `ChooseTurnDiscard` are not in `discard` and cannot be recycled into that same draw
- the card discarded by `ChooseTurnDiscard` enters `discard` only after that draw sequence has already completed

If the player has available hand space but the deck plus recyclable discard pile cannot satisfy the required draw count, report an engine error. Do not perform a partial draw and do not create a pending choice.

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

A successful pass consumes the turn's action, emits `ActionPassed { player, reason }`, and advances to `TurnDraw`.

### 13. Active Window Completion

Do not require or event-log an explicit `EndActiveWindow` command.

`ActiveWindow` accepts zero or more active-effect commands. The first valid action command implicitly closes the active window and starts action resolution.

Implications:

- successful active effects emit their own events
- failed active-effect validation emits no events
- there is no `ActiveWindowEnded` event
- replay derives active-window completion from the first action event in that turn
- `PassAction` is also an action command and therefore can implicitly close `ActiveWindow`

### 14. Public Phase Model

Use a single public input phase for active effects and the turn action:

```rust
enum Phase {
    TurnStart,
    Main,
    TurnDraw,
    TurnDrawDiscardChoice,
    TurnEnd,
}
```

`Main` allows:

- zero or more active-effect commands
- exactly one action command

Successful action commands immediately close the `Main` phase and advance toward turn draw. Action commands include `PerformFormation` and `PassAction`.

Rulebook timing names such as `ActiveWindow` and `Action` remain useful internally and in event metadata, but they are not separate public phases that require separate player commands.

### 15. Automatic Advancement Through Non-Decision Phases

The public API should stop only when player input is needed.

Non-decision phases such as `TurnStart`, `TurnDraw` without a discard choice, and `TurnEnd` are advanced by the engine automatically while still emitting events.

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

- `Main`, when the current player may use active effects or perform an action
- `TurnDrawDiscardChoice`, when the current player must choose a discard
- terminal game-over state, if added

After `ChooseTurnDiscard`, the engine may automatically process `TurnEnd`, advance turn order, run the next player's `TurnStart`, and stop at the next decision point.

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

The online adapter stores a pending command draft only when `PerformFormation` produces an `EffectGenerated` choice that suspends formation resolution. The draft preserves the original command context until the choice continuation completes. `TurnDrawDiscard` is normal turn completion after the action has resolved, so it must not create or continue a formation command draft.

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

### 21. Previous-Formation Elemental Context

Five-element interaction applies only when the previous player's immediately
preceding formation resolved as a five-element attack. An older elemental attack
does not remain eligible after that player performs a physical attack, special
attack, or spell.

```rust
struct LastElementalAttack {
    element: Element,
    resolved_turn: u64,
}
```

The last-formation state stores the performed formation identity and its resolved
category/effect separately. Five-element interaction reads the resolved category
of the previous player's latest formation. Class change that copies a five-element
attack therefore counts as that copied element, while still retaining `幻化` as
its formation identity.

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

Attack resolution events should include the point breakdown:

```rust
GameEvent::AttackResolved {
    attacker: PlayerId,
    target: PlayerId,
    formation_id: FormationId,
    point_breakdown: AttackPointBreakdown,
}
```

This supports deterministic replay, focused tests, and UI/debug display of how final damage was derived.

### 23. Five-Element Interaction Rules

Five-element interaction is resolved from the current attack element against the
element of the previous player's immediately preceding formation, when that
formation resolved as a five-element attack.

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

Once a formation is performed, its resulting effects are treated as one atomic,
simultaneous resolution from the players' perspective. Event emission order is an
implementation and replay detail, not an additional observable rule. Tests should
assert the resolved game state rather than require an event sequence, except where
the rules explicitly make an intermediate choice or separate action observable.

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
    outcome: PassiveOutcome::NoEffect { reason: PassiveNoEffectReason::NotASpell },
}
```

Defense (`防禦`) applies only to incoming attacks. If it flips against an incoming spell, pass action, or other non-attack action, it is discarded with no effect.

Defense prevents only the attack's damage. Other effects of the performed attack
still resolve; in particular, Five Streams Unite still grants its turn-draw bonus.

If seal applies to an incoming passive spell cover action, it does not immediately discard or reveal the incoming covered passive cards.

Instead, the incoming passive remains covered and is marked sealed. When that covered passive later flips at its own trigger timing, it resolves as no effect and is then discarded normally.

This prevents a covered passive from being revealed early and preserves the next player's ability to make decisions, such as choosing class change (`幻化`), without knowing the covered card identity.

The sealed covered passive uses the same public view behavior as a normal covered passive. The engine stores the sealed marker internally for later resolution, but public/player views do not expose a separate sealed marker beyond the normal covered-card visibility rules.

Each player can have at most one pending covered passive. Under normal turn flow, a player's covered passive must flip on the next player's action timing before that player can cover another passive.

If a command attempts to cover a passive while that player already has a pending covered passive, report an error. Emit no event and do not consume the action.

Empty City (`空城`) is a covered passive with no additional action modification.
It still consumes the formation cards and turn action, occupies the player's one
covered-passive position, flips at the normal trigger timing, and moves its cards
to discard. Its no-effect outcome is intentional rather than an unknown-passive
fallback. Represent that outcome explicitly as
`PassiveNoEffectReason::EmptyCity`. User-facing records should say only
`空城翻開`; they must not add redundant wording about producing no effect.

Class change (`幻化`) is a basic formation, not a special standalone action command and not a special formation category/tag.

It should be handled through the normal `PerformFormation` pipeline like other formations. It consumes the turn action, triggers covered passives at the usual next-player action timing, and uses formation category/effect rules to determine whether any passive applies.

A player with a covered passive necessarily used that passive as their latest
formation. Therefore, a next-player class change cannot both trigger that passive
and copy an older attack from the same player. Treat that combination as
unreachable rather than adding an interaction rule or test for it.

Class change preserves its own formation identity and name while copying the
previous formation's resolved category and effect. The last-formation state must
therefore store formation identity separately from the resolved effect plan. A
later class change copies that resolved category and effect, so class-change
chains continue to reproduce the original copied behavior even though every link
is still displayed and recorded as `幻化`.

Class change does not copy a spell's active/passive type because that type controls
how the formation is performed. Class change remains an active spell: its two
Earth cards are shown face up and discarded through the active-spell procedure.
When it copies Defense, Seal, Countershock, or another delayed counter effect, the
copied effect still waits for and modifies the next player's action, but it is
public and has no covered cards. Delayed counter state must therefore be modeled
separately from covered-passive card state.

Copying Empty City records Empty City as the resolved effect but creates no
delayed counter because Empty City has no effect to establish.

When the copied effect uses a card-based formula, each class change evaluates that
formula from its own two submitted Earth cards. It copies the formula, category,
and effect behavior, not the previous formation's already calculated amount.
For a copied single-card elemental strike, `level + 4` means the submitted class
change cards' level sum plus 4; it must not read only one of the two Earth cards.

Copying Five Streams Unite (`五流歸一`) includes its complete effect. Resolve its
damage from the target's current hand count and grant the class-change player the
formation's next-draw bonus. The bonus is not limited to directly submitting the
original five-card formation.

Do not add a separate `ChangeClass` command unless a future rule module introduces a genuinely non-formation class-change action.

Formation category is limited to the official two categories:

```rust
enum FormationCategory {
    Attack,
    Spell,
}
```

Passive spells are represented as spells whose `effect_id` points to an `EffectDef` with a delayed/covered plan, not as a separate top-level category.

Do not add separate categories or tags such as `ClassChange`, `PhysicalAttack`, or `ElementalAttack` unless they are needed by a concrete rule. Differences such as elemental attack interactions, passive spell covering, or class-change behavior should be represented by effect definitions under the official `Attack`/`Spell` category model.

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

Formation-use baseline zone movement is owned by the pipeline, not by individual effect resolvers.

Effect intents describe additional consequences only, such as drawing cards, discarding selected cards, changing shields, changing statuses, modifying actions, or requesting further choices.

Effect-generated pending choices are required in the first version because base formations use them.

Only one pending choice may be active at a time. When an effect creates a pending choice, the engine stops until the required player submits the corresponding choice command.

Effect choices must include enough serialized continuation data to resume deterministic resolution after the choice:

```rust
enum PendingChoiceKind {
    TurnDrawDiscard {
        drawn_cards: Vec<CardInstanceId>,
        allowed_discards: Vec<CardInstanceId>,
    },
    EffectChoice {
        source_effect: EffectId,
        chooser: PlayerId,
        options: ChoiceOptions,
        continuation: ResolutionContinuation,
    },
}
```

`ResolutionContinuation` must be serializable and replay-safe. It cannot contain closures, trait objects, borrowed references, or non-deterministic runtime state.

Pending choice invariant:

```rust
pending_choice: Option<PendingChoice>
```

Only one pending choice may exist at a time.

If a resolution needs multiple choices, it presents them sequentially. The first choice becomes `pending_choice`; remaining choice requirements are stored in `ResolutionContinuation`. After the player answers, the continuation resumes and may present the next choice.

Creating and answering a pending choice are both game events:

```rust
GameEvent::ChoiceRequested {
    choice_id: ChoiceId,
    player: PlayerId,
    kind: PendingChoiceKind,
}

GameEvent::ChoiceMade {
    choice_id: ChoiceId,
    player: PlayerId,
    selection: ChoiceSelection,
}
```

`ChoiceRequested` must include enough serialized data to reconstruct the pending choice and its continuation during replay.

Canonical events may contain hidden information required for deterministic replay, including hidden card ids, complete choice options, and serialized continuations.

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

The game ends when a team's HP reaches 0 or lower.

```rust
enum GameStatus {
    Ongoing,
    Finished { winner: Winner },
}

enum Winner {
    Team(TeamId),
    Draw,
}
```

2-player mode still uses teams: each player belongs to their own single-player team. If a team's HP reaches 0 or lower, the other team wins.

If the same resolution causes all opposing teams to reach 0 or lower at the same time, the result is `Winner::Draw`.

Once `GameStatus::Finished` is reached, gameplay commands are rejected.

HP state is clamped to 0 and never stored as a negative value.

```rust
new_hp = max(0, old_hp - damage)
```

Team HP is also capped at that match's initial HP. Every recovery source uses the
same cap, including Generating Formation, Return to Origin, generating elemental
attacks, and generating attacks divided by Countershock.

HP change events should preserve enough detail for audit/debug:

```rust
GameEvent::HpChanged {
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

Deck, hand, discard, covered passives, and other zones store `CardInstanceId`.

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
    Permanent,
}
```

Do not include `UntilNextAction` in the first version. Next-action passive behavior is modeled by pending covered passive state, not by generic status duration.

Avoid generic `remaining_turns` / `remaining_rounds` counters as the core model because they are ambiguous in multiplayer and team mode. Rule resolvers can translate rule text into explicit expiry timing.

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

The canonical event log already contains automatic events such as initial deal, discard recycling, choice requests, and status expiration. Recomputing them during replay could duplicate events or diverge across ruleset versions.

Canonical `GameEvent` and `PendingChoice` payloads are persisted record formats and participate in replay verification. Their shape must not change without an explicit migration for existing records. Data needed only by a client, such as an effect choice's required card count, belongs in the Web projection and is derived from canonical state.

### 34. Event Granularity

Use semantic events that include explicit replayable deltas.

Avoid events that are too vague to replay without recomputing rules:

```rust
GameEvent::FormationPerformed { formation_id: FormationId }
```

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
    used_cards: Vec<CardInstanceId>,
    point_breakdown: AttackPointBreakdown,
    shield_change: Option<ShieldChangeDelta>,
    hp_change: Option<HpChangeDelta>,
    card_moves: Vec<CardMoveDelta>,
}
```

This keeps the log understandable while allowing `apply_event` to mutate state directly from recorded facts rather than recomputing rule outcomes.

A single command or automatic advancement may emit multiple semantic events.

Example attack resolution may emit:

```rust
GameEvent::PassiveFlipped { ... }
GameEvent::AttackResolved { ... }
GameEvent::TurnDrawStarted { ... }
GameEvent::ChoiceRequested { ... }
```

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

- deck plus recyclable discard cannot satisfy a required draw
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
