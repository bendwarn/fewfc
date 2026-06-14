> 規則依據：官方「遊戲規則（完整規則書）」中的回合流程、施展陣法步驟、目標定義、被動術式（蓋牌）觸發點、以及基礎規則陣法列表。 ([cfecards.org][1])

---

# SKILL: CFECards Rules Engine (Turn/Action/Formation)

## 0) Goal

Build a deterministic rules engine for CFECards that:

- Enforces fixed **turn flow** (TurnStart → ActiveEffects → Action → TurnDraw → TurnEnd) ([cfecards.org][1])
- Supports **Action** types: `PerformFormation`, `ChangeClass` (and extensible “other rule-defined actions”) ([cfecards.org][1])
- Implements **Formation resolution pipelines** for:
  - Attack
  - Active Spell
  - Passive Spell (face-down “cover”, then auto-flip on next player’s action) ([cfecards.org][1])

- Supports both:
  - **2-player** mode
  - **4+ players team mode**: players split into 2 teams, sit alternately in a circle (“上家/下家” based on action order), team life applies ([cfecards.org][1])

- Provides persistence:
  - Snapshot save/load
  - Event-log save/load (recommended) with deterministic replay

---

## 1) Canonical Concepts

### 1.1 Turn & Phase (Fixed Flow)

Each player’s action constitutes one turn; after turn ends, action passes to 下家 (next player). ([cfecards.org][1])

**Turn phases (required):**

1. `TurnStart` — “回合開始時/輪到時/每回合” triggers occur ([cfecards.org][1])
2. `ActiveWindow` — player may activate various “主動效果”; can skip; order is arbitrary ([cfecards.org][1])
3. `Action` — exactly one action per turn (unless no hand / cannot act by rule) ([cfecards.org][1])
4. `TurnDraw` — exactly one “回合抽牌” per turn, base draw=2, uses discard rule (draw N+1 then discard 1) ([cfecards.org][1])
5. `TurnEnd` — end-of-turn timing; then next player’s turn begins ([cfecards.org][1])

### 1.2 Seating & Targets

- 上家 = the player who acted immediately before the current player ([cfecards.org][1])
- 下家 = the player who will act immediately after the current player ([cfecards.org][1])
- “方” targets (我方/對方) exist for team-based effects ([cfecards.org][1])

### 1.3 Formation Types

Formation major categories:

- Attacks (include 五行攻擊, 物理攻擊, 特殊攻擊) ([cfecards.org][1])
- Spells: Active Spell and Passive Spell ([cfecards.org][1])
  Passive Spell is played face-down (“蓋牌”), then **auto flips and resolves at next player’s next Action timing**; lasts one turn; if it cannot affect that action, it does not resolve its effect. ([cfecards.org][1])

---

## 2) Data Model (Minimum Required)

### 2.1 IDs

- `PlayerID` stable string
- `CardInstanceID` unique per card in match (int or uuid)
- `CardDefID` identifies the printed card/type

### 2.2 GameState

```ts
type Phase = 'TurnStart' | 'ActiveWindow' | 'Action' | 'TurnDraw' | 'TurnEnd'

interface GameState {
  // meta
  rulesetVersion: string // for save migrations
  rngSeed: number // deterministic replay
  rngCursor: number // optional; or store deck order explicitly

  // turn
  turnNumber: number
  phase: Phase
  turnOrder: PlayerID[] // circular order
  currentTurnIndex: number // index in turnOrder

  // teams
  teamOf: Record<PlayerID, 'A' | 'B' | null> // null for 2P free-for-all
  teamHP: Record<'A' | 'B', number> // for team mode
  playerHP?: Record<PlayerID, number> // if 2P or non-team variants

  // zones
  deck: CardInstanceID[]
  hand: Record<PlayerID, CardInstanceID[]>
  discard: CardInstanceID[]

  // hidden/passive
  coveredPassiveByPlayer: Record<PlayerID, CardInstanceID[] | null>
  // meaning: cards that player covered on their previous turn (face-down)

  // combat context (for 五行 interactions)
  lastAttackElementByPlayer: Record<PlayerID, string | null> // store element if last action was 五行 attack
  shield: Record<PlayerID, number> // 防護罩 value (or per-team depending on rules module)

  // status effects with durations (“回合/輪”等)
  statuses: Array<StatusEffect>
}
```

**Notes:**

- Hand limit 5 (affects draw) ([cfecards.org][1])
- Attack target (damage part) is always 上家; extra effects may have other targets ([cfecards.org][1])

### 2.3 StatusEffect (Duration System)

Support “回合” and “輪” style durations (module-defined), and common “until next action” types.

```ts
interface StatusEffect {
  id: string
  owner: PlayerID | 'TeamA' | 'TeamB'
  kind: string // e.g. "CannotAct", "CannotDraw", "DrawPlus", etc.
  value?: number
  duration: { type: 'turns' | 'rounds' | 'untilNextAction'; remaining: number }
}
```

---

## 3) Engine Architecture

### 3.1 Core loop = FSM

- Only the engine advances `phase`.
- UI/AI/network only submits **Commands**.

Phase progression:

- `TurnStart` → apply scheduled triggers
- `ActiveWindow` → accept 0..N “active effects” commands (module-defined), then player ends window
- `Action` → accept exactly 1 action command (unless rule says cannot)
- `TurnDraw` → execute draw pipeline
- `TurnEnd` → apply end triggers → advance currentTurnIndex (circular) → `TurnStart`

This directly matches the rulebook’s fixed turn process. ([cfecards.org][1])

### 3.2 Commands (Command Pattern)

All gameplay changes must come from validated commands:

```ts
interface Command {
  type: string
  player: PlayerID
  payload: any
}
```

Required command types:

- `EndActiveWindow`
- `PerformFormation` (attack/active/passive)
- `ChangeClass` (if Hero module enabled) ([cfecards.org][1])
- `ChooseTurnDiscard` (for TurnDraw discard rule) ([cfecards.org][1])

Optional / module:

- Spirit skill, etc. (as “Active effects” in ActiveWindow) ([cfecards.org][1])

Validation rules:

- Reject if `command.player != currentPlayer` unless explicitly allowed by module.
- Reject actions outside valid phase.

---

## 4) Formation Resolution Pipelines (No Chain/Stack)

### 4.1 Shared Preliminaries

Formation has:

- `name / castPattern / category / effect` ([cfecards.org][1])
  Cast pattern parsing (e.g., “金金火水”, “三張牌連續相生”) is part of `FormationMatcher`.

### 4.2 Attack Pipeline (Must follow)

On `PerformFormation` where category=Attack:

1. Reveal formation cards; declare attack name ([cfecards.org][1])
2. Compute points
3. If 上家 covered a passive spell last turn: **flip and resolve it**, then discard those cards ([cfecards.org][1])
4. Apply damage & extra effects:
   - Damage equals points to 上家; if target has shield, shield takes damage ([cfecards.org][1])
   - If 上家 last turn used 五行 attack, apply 相生/相剋/相抵 modifications; if 上家 has shield, no 生剋抵 ([cfecards.org][1])

5. Move used formation cards to discard

### 4.3 Active Spell Pipeline

On category=Active Spell:

- Resolve immediately when played (same concept as attack) ([cfecards.org][1])
- Include the same “flip 上家 passive if covered” step when rulebook specifies it for spell resolution (implement as shared “before resolve: flip previous-player passive if exists” hook, enabled for both attacks and active spells if required by rules text). ([cfecards.org][1])

### 4.4 Passive Spell Pipeline (Cover & Auto-Flip)

On category=Passive Spell:

- When performed: place cards face-down as `coveredPassiveByPlayer[currentPlayer]=cards` and end action ([cfecards.org][1])
- At **next player’s next Action timing** (i.e., when they begin resolving their action), auto-flip that passive and resolve it; then discard it ([cfecards.org][1])
- If it cannot affect that action, do not apply its effect (but still discard, as it “must trigger next turn” semantics). ([cfecards.org][1])

---

## 5) Rule Modules (Pluggable)

Core engine knows only:

- turn FSM
- seating/targets
- command dispatch
- formation pipelines (generic)
- draw/discard rule
- persistence primitives

Modules register:

- Formation definitions (base 25 formations, plus expansions)
- Extra commands/actions
- Extra triggers at specific timings

### 5.1 BasicRuleModule (Required baseline)

Provide the 25 base formations and their formulas/keywords (examples include 防禦/封印 as passive “反制” concept). ([cfecards.org][2])

Important: “反制” effects are conceptually **no-target** defensive traps even if text mentions 下家; treat them as owner-attached passives. ([cfecards.org][1])

### 5.2 TeamModeModule

Enable:

- team seating interleaving (turnOrder built accordingly)
- team HP damage accounting (damage to a player reduces their team HP) ([cfecards.org][1])
- “方” targets resolution (我方/對方) ([cfecards.org][1])

---

## 6) Persistence (Save/Load)

### 6.1 Snapshot Save (Simple)

Store:

- `rulesetVersion`
- `rngSeed` (+ either `rngCursor` or full `deck` order)
- full `GameState`
- pending covered passives (hidden info)
- pending “choose discard” prompt state if game can be saved mid-draw

### 6.2 Event Log Save (Recommended)

Store:

- `rulesetVersion`
- `rngSeed`
- `events: GameEvent[]`

Each accepted command emits 1+ events:

- `TurnStarted(player)`
- `ActiveWindowEnded(player)`
- `FormationPerformed(player, formationId, cardInstanceIds, declaredTargets?, computedPoints?)`
- `PassiveCovered(player, cardInstanceIds)`
- `PassiveFlipped(owner, resolvedAgainstPlayer, resultSummary)`
- `TurnDrawn(player, drawnCardIds, discardedCardId)` (draw rule requires N+1 draw then choose discard) ([cfecards.org][1])
- `TurnEnded(player)`

Replay rule:

- Initialize empty state from seed + initial deal setup (first player gets 4, others 5) ([cfecards.org][1])
- Apply events sequentially; must reproduce identical state

Determinism:

- Prefer storing full deck order in first event (`DeckShuffled(order=...)`) to avoid RNG implementation differences across platforms.

---

## 7) Testing Checklist

- Turn flow always hits all phases even if player does nothing (except where rules allow skipping actions due to no hand / cannot act). ([cfecards.org][1])
- “主動效果” can be done in any order or skipped. ([cfecards.org][1])
- Exactly one action per turn. ([cfecards.org][1])
- Turn draw base=2 and uses discard rule “draw N+1 choose 1 discard”. ([cfecards.org][1])
- Passive cover triggers on next player’s action timing and lasts one turn. ([cfecards.org][1])
- Attack pipeline flips 上家 passive before applying damage. ([cfecards.org][1])
- 五行生剋抵 only applies vs 上家 last turn 五行攻擊, and is disabled if target has shield. ([cfecards.org][1])
- Team seating interleaving and “方” targets work. ([cfecards.org][1])

---

如果你要我再往下「更可直接寫程式」一點，我可以把上面 skill 轉成：

- TypeScript 的完整型別檔（`GameState.ts`, `commands.ts`, `events.ts`, `modules.ts`）
- 以及 `resolveAttack/resolveActiveSpell/resolvePassiveSpell` 的骨架程式碼（只留 TODO 給每張陣法的效果），讓 Codex 直接填實作。

[1]: https://www.cfecards.org/rule/latest/you-xi-gui-ze '五行戰鬥牌官方網站 - 遊戲規則（完整規則書）'
[2]: https://www.cfecards.org/rule/latest/basicrule '五行戰鬥牌官方網站 - 基礎規則'
