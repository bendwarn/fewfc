# CFECards Rules Engine Decisions

本文件記錄 Rust 實作前釐清的實作決策。

## Confirmed

### 1. Implementation Boundary

將規則引擎建成純 Rust 函式庫 crate。

初始實作只負責確定性遊戲規則：

- 驗證命令；
- 推進回合／階段狀態；
- 結算陣法；
- 發出事件；
- 支援快照持久化；
- 支援事件日誌重播。

初始實作不包含介面、網路或 CLI；日後可在函式庫外加入介面卡。

### 2. Architecture

使用 Clean Architecture 邊界。

領域核心擁有遊戲概念及規則不變條件。應用程式服務協調命令與重播；基礎設施處理序列化、儲存、RNG／牌堆初始化及未來介面卡。

領域層不得依賴介面、儲存、網路或框架事項。

### 3. Crate And Module Shape

從單一 Rust 函式庫 crate 開始，透過內部模組邊界落實 Clean Architecture。

預計模組結構：

```text
src/
  domain/          # GameState, PlayerId, CardId, Phase, Formation, rule invariants
  application/     # Command handling, turn orchestration, replay service
  rules/           # BasicRuleModule, TeamModeModule, formation definitions/effects
  ports/           # traits: EventStore, SnapshotStore, RngSource if needed
  infrastructure/  # serde persistence, deterministic deck/shuffle helpers
```

初期不拆成 Rust workspace；若專案擴大，模組邊界應使未來拆成多個 crate 保持容易。

### 4. Persistence Source Of Truth

以事件日誌作為正規遊戲紀錄；快照是加快載入的選用快取／檢查點資料。

每個已接受命令發出一個或多個 `GameEvent`。`GameState` 是由初始設定及重播事件推導的投影。

主要持久化結構：

```rust
struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    latest_snapshot: Option<GameSnapshot>,
}
```

載入可用 `latest_snapshot` 最佳化，但重播仍是權威行為。測試必須驗證直接命令執行與重播產生相同狀態。

### 5. Randomness And Deck Order

在事件日誌持久化完整準備後牌序。種子可儲存為中繼資料／除錯情境，但重播不得依賴任何 RNG 演算法。

開局事件結構：

```rust
GameEvent::DeckPrepared {
    deck_order: Vec<CardInstanceId>,
}
```

重播直接使用 `deck_order`；測試可提供固定牌序而不呼叫 RNG。

日後若加入種子式牌堆生成，只在建立初始遊戲紀錄時執行。一旦發出 `DeckPrepared`，事件日誌中的牌序即為權威。

### 6. Players, Teams, And HP Ownership

雙人及團隊模式使用同一目標／生命值模型。

生命值屬於隊伍：

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

雙人模式中每位玩家各屬一個一人隊伍；團隊模式中多位玩家可屬同一隊伍。

行動順序一律依玩家決定。傷害先以玩家為目標，再結算至該玩家的 `TeamId` 以改變生命值。

### 7. Hidden Information And Covered Passives

內部領域狀態儲存完整隱藏資訊；公開／玩家專屬視圖在邊界過濾隱藏資訊。

內部狀態範例：

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

每位玩家擁有一個陣法區，至多包含一個陣法。蓋牌是該區陣法的 `FaceDownWaiting` 狀態，不是獨立牌容器或區域。因此新行動玩家自己的陣法區可有正面陣法，同時上家的蓋牌仍占用上家陣法區。

`FormationCommitted` 與 `PassiveCovered` 事件儲存真實 `CardInstanceId` 值，使重播保持確定且完整。

依觀看者產生外部視圖：

- 陣法區擁有者可看見自己的覆蓋牌；
- 其他玩家只看見隱藏牌數量。

隱藏資訊過濾是 API／呈現事項，不得削弱領域狀態或重播模型。

### 8. Passive Reveal Timing

蓋牌以一個正規觸發時點建模：

```rust
enum PassiveTriggerTiming {
    NextPlayerActionStart,
}
```

玩家 A 在行動時覆蓋被動術式，下家行動結算開始時檢查。若行動順序中 B 為下家，則 A 是 B 的上家（`上家`）；因此規則書「攻擊／術式結算前翻開上家蓋牌」即為同一時點。

結算順序：

1. 驗證 B 的完整行動命令。驗證失敗不發出事件，也不改變狀態。
2. 命令為 `PerformFormation` 時，發出 `FormationCommitted`，將實體牌從 B 的手牌移至 B 的陣法區。成功行動命令進入 `Action`；成立之後不回滾。
3. 引擎檢查 A 的陣法區是否有蓋牌。
4. 若有則翻開，並嘗試影響 B 的行動。
5. 不論效果是否適用，蓋牌均從 A 的陣法區移至適用的來源棄牌堆。
6. 除非蓋牌改變或防止結果，否則 B 的行動繼續。

不為攻擊與主動術式分別實作蓋牌觸發系統。

### 9. Validation Failure Versus Resolution Outcome

驗證失敗不是遊戲事件。

命令驗證失敗時，引擎回傳錯誤，不改變狀態、不消耗行動，也不附加事件日誌。

驗證失敗包括：

- 階段錯誤；
- 玩家錯誤；
- 缺少牌；
- 陣法無效；
- 非法目標宣告。

驗證成功且行動結算開始後，行動即成為遊戲歷史並消耗該回合行動，即使最終結果被防止或無效果亦然。

結算結果記錄在實際發生的語意結算中，例如攻擊結算：

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

適用效果恰產生一個結果。`grounds` 包含每個各自足以造成無效的依據；其順序僅為確定性序列化順序，絕非主要原因或領域優先權。收集依據前先檢查適用性，因此防禦面對術式而不適用時只記錄 `NotAnAttack`，不記錄無關免疫依據。

這保留每回合恰好一次行動的語意，而不把無效命令當成歷史事實。不新增抽象 `FormationUseCompleted` 事件：陣法正常完成以實際 `FormationCardsDiscarded` 或 `PassiveCovered` 事實表示，終局結算則以 `GameEnded` 結束。

### 10. Turn Draw Pending Choice

來源依據：官方規則 4-2.4d 抽取比將加入手牌數量多一張的牌，玩家選其中一張捨棄，之後才將其餘牌加入手牌。

實作解讀：捨棄前的牌需要既非手牌也非棄牌堆的正規區域；等待選擇是官方回合抽牌階段內的狀態，而非第六個回合階段。

回合抽牌需要待選擇事項，因為玩家抽取 `N + 1` 張牌並選其中一張捨棄。此流程完全位於正規 `TurnDraw` 階段；等待選擇是該階段內狀態，不是另一階段。

使用整局共用的一個回合抽牌區：

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

流程：

1. 進入 `TurnDraw`。
2. 發出 `CardsDrawnForTurnDiscardChoice`，將 `N + 1` 張牌從適用牌堆移入回合抽牌區並建立棄牌待選擇事項。這些牌不在玩家手牌中。
3. 階段仍為 `TurnDraw` 時接受 `ChooseTurnDiscard`。
4. 發出一個原子 `TurnDrawResolved { discard, kept_cards }`。所選牌先進入適用的來源棄牌堆，其餘牌再加入玩家手牌；兩次移動之間不存在玩家輸入或正規重播狀態。
5. 清空抽牌區並進入 `TurnEnd`。

這支援抽牌途中儲存／載入、介面等待狀態、線上遊戲及確定性重播。

快照為快取，可從正規事件重建。

### 11. Draw Limits, Hand Limit, And Discard Shuffles

手牌上限為五張。玩家手牌已達上限時，略過回合抽牌，不建立待選擇事項。

否則回合抽牌依「抽 `N + 1` 張，再選一張剛抽的牌捨棄」的規則意圖進行，其中 `N = min(base_draw, available_hand_space)`。

因此玩家至少有一個手牌空位時，回合抽牌區可有 `N + 1` 張牌等待，而既有手牌不變；玩家不會暫時持有超過上限的手牌。

`allowed_discards` 只能包含回合抽牌區中的牌；抽牌前已在玩家手中的牌不能作為 `ChooseTurnDiscard` 合法選項。

牌堆不足時先**洗棄牌**：洗勻完整適用棄牌堆，將所有洗後牌放到同一牌堆底，再繼續抽牌。牌堆即使非空，只要少於抽牌所需張數仍須洗棄牌。

牌堆方向：

- `deck[0]` 是牌堆頂及下一張抽取的牌。
- `deck.last()` 是牌堆底。

洗棄牌使用可信任隨機性決策邊界。正規請求及結果區分將完整棄牌堆洗入牌堆底，與重新排列牌堆中已有的牌；重播使用記錄順序，不重新執行 RNG。續接描述洗牌後恢復什麼，不分類發生的是哪種洗牌。

依牌目前區域判定是否可參與洗棄牌：

- `TurnDraw` 前已移入棄牌堆的陣法牌可參與同次回合抽牌期間的洗棄牌；
- 回合抽牌區中的牌不在棄牌堆中，不能參與同次洗棄牌；
- `ChooseTurnDiscard` 所選牌只在套用 `TurnDrawResolved` 時進入適用的來源棄牌堆。

玩家手牌仍有空位，但牌堆加上適用棄牌堆不足以滿足所需抽牌數時，回報引擎錯誤；不部分抽牌，也不建立待選擇事項。

### 12. Passing The Action Phase

不自動略過 `Action` 階段，使用明確命令：

```rust
Command::PassAction {
    reason: PassActionReason,
}

enum PassActionReason {
    NoCardsInHand,
    CannotActByStatus,
}
```

驗證：

- 玩家能進行合法行動時，`PassAction` 驗證失敗；
- 玩家沒有手牌時，`PassAction { reason: NoCardsInHand }` 合法；
- 玩家具有 `CannotAct` 狀態時，`PassAction { reason: CannotActByStatus }` 合法。

成功略過會進入 `Action`、消耗回合行動、處理並捨棄上家蓋牌，並發出 `ActionPassed { player, reason }`。除非該行動使遊戲結束，否則推進至 `TurnDraw`。

### 13. Active Effects Completion

不要求也不在事件日誌記錄明確 `EndActiveEffects` 命令。

`ActiveEffects` 接受零個或多個主動效果命令；第一個有效行動命令結束 `ActiveEffects` 並進入 `Action`。

其結果：

- 成功主動效果發出各自事件；
- 主動效果驗證失敗不發出事件；
- 不存在 `ActiveEffectsEnded` 事件；
- 重播從該回合第一個行動事件推導完成；
- `PassAction` 也是行動命令，因而進入 `Action`。

### 14. Public Phase Model

來源依據：官方規則 4-2 依序定義回合開始、主動效果、行動、回合抽牌及回合結束。

實作解讀：直接將五個規則階段公開為正規 Phase 值；待選擇事項暫停所屬階段，不建立新階段。

使用官方五階段回合流程作為正規公開階段模型：

```rust
enum Phase {
    TurnStart,
    ActiveEffects,
    Action,
    TurnDraw,
    TurnEnd,
}
```

`ActiveEffects` 允許：

- 零個或多個主動效果命令；
- 第一個已接受行動命令，它會進入 `Action`。

`Action` 使已接受命令經過上家蓋牌處理、全部效果及每個效果產生的待選擇事項；行動完成或遊戲結束前不推進至 `TurnDraw`。行動命令包含 `PerformFormation`、轉職及 `PassAction`。

回合抽牌棄牌待選擇事項留在 `TurnDraw`；此選擇及效果產生的選擇均不新增階段。

### 15. Automatic Advancement Through Non-Decision Phases

公開 API 只在需要玩家輸入時停止。

引擎自動推進 `TurnStart`、`ActiveEffects`、`Action`、`TurnDraw` 及 `TurnEnd` 中的非決策工作，並持續發出事件。

建議 API：

```rust
fn apply_command(
    state: &mut GameState,
    command: Command,
) -> Result<Vec<GameEvent>, CommandError>;

fn advance_until_decision(
    state: &mut GameState,
) -> Result<Vec<GameEvent>, EngineError>;
```

引擎停止於：

- `ActiveEffects`：目前玩家可使用主動效果或進行行動；
- `Action`：其待選擇事項需要玩家輸入；
- `TurnDraw`：其棄牌待選擇事項需要玩家輸入；
- 終局 `Finished` 狀態。

`ChooseTurnDiscard` 後，引擎可自動處理 `TurnEnd`、推進行動順序、執行下一玩家的 `TurnStart`，並停在下一個決策點。

### 16. Command Handling And Event Application

暫定使用事件溯源命令流程：

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

命令處理驗證並決定事件。狀態透過 `apply_event` 修改，使即時執行與重播共用狀態轉換路徑。

驗證失敗回傳錯誤，不發出事件，也不修改狀態。

只有 `PerformFormation` 產生 `EffectGenerated` 選擇並暫停所屬 `Action` 時，線上介面卡才儲存待處理命令草稿。草稿保留原命令情境，直到選擇續接完成。回合抽牌棄牌選擇屬於 `TurnDraw`，因此不得建立或延續前一行動命令草稿。

### 17. Formation Matching

可行情況下使用資料驅動陣法組合方式；無法清楚表示為資料的陣法使用自訂匹配鉤子。

建議模型：

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

`SequentialGenerating` 表示連續五行相生順序，例如 `木 -> 火 -> 土`。

`SequentialOvercoming` 表示連續五行相剋順序，例如 `木 -> 土 -> 水`。

為方便玩家，陣法匹配不要求提交牌依順序排列。匹配器應將提交牌視為集合／多重集合，判定是否存在符合組合方式的合法順序。

匹配邏輯只判定牌是否組成合法陣法；效果規劃／結算另由 `effect_id` 分派。

### 18. Formation Declaration

同一組牌可匹配多個陣法時，玩家或上層必須明確宣告施展哪個陣法。

```rust
Command::PerformFormation {
    player: PlayerId,
    formation_id: FormationId,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
}
```

引擎驗證提交牌可組成宣告的 `formation_id`，不代替玩家推測或自動選取陣法。

這使存在多個合法解讀時，重播、介面及 AI 行為維持確定性。

#### 18.1 Formation Commitment And Completion

來源依據：官方規則 5-2 與 5-3 在效果結算前展示已接受陣法，正常完成後捨棄陣法牌；規則 5-4 則使已施展被動術式覆蓋，等待下家行動。

實作解讀：每位玩家擁有一個陣法區；陣法驗證成功後，在處理任何傳入反制效果前先將實體牌提交至該區。

提交任何狀態前完成全部 `PerformFormation` 驗證。驗證成功時發出 `FormationCommitted`，以原子方式將所有提交實體牌從施展玩家手牌移至其陣法區並進入 `Action`。攻擊及立即術式正面提交；被動術式反面覆蓋提交。

成立在該行動中不可逆。反制、防止或無效果不會將牌退回手牌。效果產生的待選擇事項暫停同一 `Action`，已成立陣法仍留在其區域。

非被動陣法及其所有依序選擇與效果完成後，發出 `FormationCardsDiscarded`，將其實體牌從陣法區直接移至適用的來源棄牌堆，接著進入 `TurnDraw`。蓋牌則發出 `PassiveCovered`，在陣法區保持反面覆蓋並進入 `TurnDraw`。不新增抽象 `FormationUseCompleted` 事件。

若行動發出 `GameEnded`，上述兩條一般完成路徑均不執行。已成立陣法留在陣法區，因為規則停止後續處理，而非執行終局清理。

### 19. Attack Base Damage Target

攻擊陣法基本傷害一律以依行動順序解析的上家（`上家`）為目標。

玩家不宣告攻擊基本傷害目標，引擎自行推導：

```rust
let damage_target = previous_player(current_player, turn_order);
```

`PerformFormation` 的 `declared_targets` 僅保留供需要選擇的附加效果使用。引擎依宣告的 `FormationDef` 驗證目標數量及類型。

### 20. Declared Targets

`declared_targets` 只儲存具體玩家選擇：

```rust
enum TargetDecl {
    Player(PlayerId),
    Team(TeamId),
    Card(CardInstanceId),
}
```

上家、下家、自己、我方或對方等由規則推導的語意目標不由玩家提交，而由陣法／效果邏輯從目前狀態解析。

這使事件日誌聚焦實際玩家選擇，同時讓規則定義的目標維持為狀態的確定性投影。

### 21. Previous-Turn Formation Query

來源依據：規則 5-2.4b 要求上家在剛結束的上回合施展五行攻擊，才適用五行生剋。基礎規則 2-2.2 與 2-2.4 對幻化複製目標採相同回合範圍；江湖規則 4-7 以相同範圍判定點水扇。

實作解讀：`last_formation_by_player` 保留為重播狀態，但這三條規則只在 `resolved_turn == turn_number - 1` 時查詢。上回合略過、轉職或沒有陣法時，沒有紀錄符合條件。回合開始觸發既有陣法效果不建立新的陣法施展；蓋牌屬於覆蓋的回合。

最近陣法狀態分別儲存施展陣法身分與已結算類別／效果。剛結束的上回合施展之幻化若複製五行攻擊，五行生剋讀取複製後屬性，但陣法身分仍為 `幻化`。

五行攻擊屬性不會被封印。合法五行攻擊施展後，即使行動其他部分受防禦效果影響，其屬性仍供五行生剋追蹤。

防止傷害、防護罩吸收或其他防禦效果不改變已結算陣法類別。驗證失敗不更新該狀態，因為沒有遊戲事件發生。

### 22. Attack Point Breakdown

明確表示五行點數修正，不將其隱藏在最終傷害中。

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

攻擊結算將點數明細與所有已結算結果一同包含：

```rust
struct AttackResolution {
    outcome: ActionOutcome,
    point_breakdown: AttackPointBreakdown,
    damage: AttackDamageResolution,
    additional_effects: ResolvedAttackEffects,
}
```

這支援確定性重播、重點測試，以及介面／除錯畫面顯示最終傷害的推導方式。

### 23. Five-Element Interaction Rules

以目前攻擊屬性對照上家剛結束的上回合所施展五行攻擊屬性，結算五行生剋。

官方關係：

- 相生順序：`金 -> 水 -> 木 -> 火 -> 土 -> 金`；
- 相剋順序：`金 -> 木 -> 土 -> 水 -> 火 -> 金`；
- 同屬性為相抵（`相抵`）。

目前攻擊屬性生上家上回合攻擊屬性時，攻擊不造成傷害，改為回復目標與攻擊點數相等的生命。

目前攻擊屬性剋上家上回合攻擊屬性時，傷害加倍。兩者相同時，傷害減半並無條件進位。

範例：

- 上回合 `金`、本次 `火`：相剋，傷害加倍；
- 上回合 `金`、本次 `土`：相生，回復目標；
- 上回合 `金`、本次 `金`：相抵，傷害減半並無條件進位；
- 上回合 `金`、本次 `木` 或 `水`：無生剋互動。

目標有防護罩時完全略過五行生剋。五行生剋改變傷害／回復結果，不改變攻擊點數。

### 24. Shield Ownership And Damage Absorption

防護罩附屬於玩家，而非隊伍。

```rust
shield_by_player: HashMap<PlayerId, Shield>

struct Shield {
    amount: i32,
}
```

攻擊基本傷害仍以上家為目標。生命值傷害結算至該玩家隊伍，但防護罩查詢依目標玩家進行。

防護罩沒有持續時間，會一直存在直到耗盡或被其他效果明確改變。

目標玩家有防護罩時：

- 略過五行生剋；
- 將攻擊傷害套用至防護罩；
- 超額傷害不穿透至玩家／隊伍生命值。

物理攻擊對防護罩造成雙倍傷害；特殊攻擊及五行攻擊使用其餘規則結算後的傷害數值。

即使傳入傷害超過防護罩點數，防護罩仍作為獨立防禦層吸收該次攻擊。防護罩點數降至零或以下時立即移除。

已有防護罩的玩家獲得新防護罩時，新點數取代舊點數，防護罩不疊加。

### 25. Passive Action Modifications

核心流程不將被動術式硬編碼為「反制即取消一切」的單一行為。每個被動效果回傳一個或多個行動修正：

```rust
enum ActionModification {
    Continue,
    CancelAction { reason: CancelReason },
    PreventDamage,
    ModifyDamage(DamageModifier),
    PreventExtraEffects,
}
```

被動效果結算流程：

1. 下家行動開始時翻開上家的蓋牌。
2. 被動效果檢查是否適用於傳入行動。
3. 效果回傳行動修正。
4. 傳入行動的結算套用這些修正。

在攻擊結算中，玩家所見的傷害及未另指定時點的全部附帶效果是單一原子、同時的結果。它們屬於一個 `AttackResolved` 事件，使實作、重播及公開視圖不能虛構可觀察順序。明確指定時點的效果留在事件外；必要中間選擇在發出事件前收集。

反震（`反震`）在防禦層吸收傷害前分攤傳入攻擊。攻擊方直接承受反彈份額；防守玩家的份額再依正常防護罩規則由其防護罩承受（若有）。防護罩不得抑制反震分攤。套用分攤期間若任一方生命值歸零，仍繼續處理直到雙方份額均套用；遊戲結束判定使用完整結算狀態，因此可能平局。

五行生剋先決定攻擊結果模式，反震再分配其數值。反震分攤結果數值，但不改變模式。因此相生使攻擊回復生命時，攻擊與防守雙方各回復自己的份額；相剋、相抵及無關屬性亦先套用修正數值再分攤。

陣法點數的小數一律無條件進位。反震以無條件進位除法分別計算雙方份額，因此 7 點攻擊使攻擊方與防守方各分得 4 點。

封印（`封印`）只適用術式（`術式`），不建模為廣泛的「術式或效果」取消規則。

封印翻開時若傳入行動是攻擊，因該行動不是術式而無效果並捨棄：

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

防禦（`防禦`）只適用傳入攻擊；面對傳入術式、行動略過或其他非攻擊行動時，翻開後無效果並捨棄。

防禦只防止攻擊傷害。已施展攻擊的其他效果仍結算；特別是五流歸一仍提供回合抽牌加成。

封印作用於傳入被動術式覆蓋行動時，不立即捨棄或翻開新覆蓋牌。新被動術式仍覆蓋並標記為已封印；日後於其觸發時點翻開、無效果並正常捨棄。

這避免提前揭露蓋牌，也使下家可在不知道蓋牌身分下作決策，例如選擇幻化（`幻化`）。

已封印蓋牌的公開視圖行為與一般蓋牌相同。引擎在內部儲存封印標記供日後結算，但公開／玩家視圖除正常蓋牌可見性規則外，不另公開封印標記。

每位玩家的陣法區至多容納一個陣法。正常回合流程下，玩家蓋牌在其能提交另一陣法前，於下家的 `Action` 翻開。

命令嘗試在玩家陣法區已有陣法時再提交陣法，應回報錯誤，不發出事件，也不消耗行動。

空城（`空城`）是沒有額外行動修正的蓋牌。它仍消耗陣法牌與回合行動、占用玩家唯一陣法區、在正常觸發時點翻開並將牌捨棄。其無效果結果是刻意設計，不是未知被動效果的遞補；以 `PassiveNoEffectGround::EmptyCity` 明確表示。玩家紀錄只應說 `空城翻開`，不得多加無效果的重複文案。

幻化（`幻化`）是基礎陣法，不是特殊獨立行動命令，也不是特殊陣法類別／標籤。

它與其他陣法一樣透過正常 `PerformFormation` 流程處理，消耗回合行動，在正常下家行動時點觸發蓋牌，並依陣法類別／效果規則判定是否適用任何被動效果。

持有蓋牌的玩家必然以上回合陣法施展了該被動術式。因此下家的幻化不能同時觸發該蓋牌，又複製同一玩家更早的攻擊；此組合視為不可到達，不新增互動規則或測試。

幻化複製上回合陣法已結算類別及效果，同時保留自己的陣法身分與名稱。因此最近陣法狀態必須分別儲存陣法身分與已結算效果計畫。後續幻化複製該已結算類別與效果，使幻化連鎖持續重現最初複製的行為，即使每一環仍顯示並記錄為 `幻化`。

幻化不複製術式的主動／被動類型，因為該類型控制陣法如何施展。幻化仍為主動術式：兩張土牌正面展示，並依主動術式程序捨棄。複製防禦、封印、反震或其他延遲反制效果時，複製效果仍等待並修改下家行動，但它是公開的且沒有覆蓋牌。因此延遲反制狀態必須與蓋牌狀態分開建模。

複製空城時，記錄空城為已結算效果，但不建立延遲反制，因為空城沒有可建立的效果。

複製效果使用基於牌的公式時，每次幻化以自己提交的兩張土牌計算公式。它複製公式、類別及效果行為，不複製上回合陣法已算出的數值。複製單張五行擊術時，`level + 4` 指所提交幻化牌的等級總和加四，不得只讀兩張土牌其中之一。

複製五流歸一（`五流歸一`）包含其完整效果：依目標目前手牌數結算傷害，並給幻化玩家該陣法的下次抽牌加成。加成不限於直接提交原本五張陣法。

不為幻化新增獨立 `ChangeClass` 命令；轉職是對應規則模組引入的獨立行動。

陣法類別限官方兩類：

```rust
enum FormationCategory {
    Attack,
    Spell,
}
```

被動術式表示為其 `effect_id` 指向具延遲／覆蓋計畫的 `EffectDef` 之術式，不是獨立頂層類別。

除非具體規則需要，不新增 `ClassChange`、`PhysicalAttack` 或 `ElementalAttack` 等獨立類別或標籤。五行攻擊互動、被動術式覆蓋及幻化行為等差異，應在官方 `Attack`／`Spell` 類別模型下以效果定義表示。

使用 `effect_id + EffectDef.plan` 而非 `FormationBehavior`：

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

`FormationDef` 識別陣法是什麼及如何匹配；`EffectDef.plan` 識別陣法如何執行。

攻擊計畫細節可區分攻擊子類型，不改變官方陣法類別：

```rust
enum AttackPlanDef {
    Elemental { element: Element, points: PointFormula },
    Physical { points: PointFormula },
    Special { points: PointFormula, special: SpecialAttackId },
}
```

五行生剋只適用 `AttackPlanDef::Elemental`；防禦適用所有 `EffectPlanDef::Attack` 計畫。

攻擊點數使用公式，而不只固定值：

```rust
enum PointFormula {
    Fixed(i32),
    PerCard { points_per_card: i32 },
    SumCardValues,
    Custom(PointFormulaId),
}
```

固定點數攻擊使用 `PointFormula::Fixed`；可變點數攻擊新增公式解析器，不改變陣法 schema。

自訂匹配器、點數公式及效果解析器必須是確定性的純解析器。

範例：

```rust
trait PointFormulaResolver {
    fn compute(&self, ctx: PointFormulaContext) -> Result<i32, RuleError>;
}
```

允許：

- 讀取陣法牌；
- 讀取牌定義；
- 讀取目前狀態快照；
- 回傳驗證結果、計算點數或效果意圖。

不允許：

- 修改 `GameState`；
- 直接在區域間移牌；
- 直接附加 `GameEvent`；
- 使用非確定性 RNG、牆上時鐘時間或 IO。

任何隨機性或玩家選擇均須透過命令與事件明確表示。

立即術式、被動術式及攻擊效果解析器均回傳宣告式意圖。流程將意圖轉為事件，`apply_event` 修改狀態。

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

陣法施展的基本區域移動由流程而非個別效果解析器負責：效果前發出 `FormationCommitted`，一般正面完成後發出 `FormationCardsDiscarded`。

效果意圖只描述額外結果，例如抽牌、捨棄所選牌、改變防護罩或狀態、修改行動或請求後續選擇。

基礎陣法會產生選擇，因此第一版即須支援效果產生的待選擇事項。

同時只能有一個待選擇事項作用中。效果建立待選擇事項時，引擎停止，直到所需玩家提交對應選擇命令。

效果選擇與隨機性請求攜帶一個可序列化結算擁有者；輸入物件本身仍僅為輸入要求：

```rust
enum PendingResolution {
    TurnDrawDiscard,
    ChaosReturnTwo,
    // ... one explicit variant for every paused rule flow
}
```

`PendingResolution` 必須可序列化且能安全重播，不能包含閉包、trait 物件、借用參照或非確定性執行期狀態。

待輸入不變條件：

```rust
pending_choice: Option<PendingChoice>
pending_randomness: Option<PendingRandomness>
pending_resolution: Option<PendingResolution>
```

同時至多存在一個輸入要求，且必須恰有與之匹配的 `PendingResolution`。公開視圖絕不暴露 `PendingResolution`。

結算需要多次選擇時依序呈現；收到答案或隨機結果後，引擎從 `PendingResolution` 恢復，並可請求下一輸入。

建立及回答效果產生的待選擇事項均為遊戲事件：

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

`ChoiceRequested` 必須包含足夠序列化資料，使重播能同時重建待選擇事項與其結算擁有者。`RandomnessRequested` 遵循相同模式。

回合抽牌使用一般待選擇生命週期，同時保留專屬牌移動事件。`CardsDrawnForTurnDiscardChoice` 建立抽牌區，接著 `ChoiceRequested` 建立待選擇事項。回答命令先發出 `ChoiceMade`，再發出 `TurnDrawResolved`，以原子方式記錄答案的棄牌及保留牌移動。`TurnDrawResolved` 內兩次移動之間不存在玩家輸入或重播狀態。

正規事件可包含確定性重播所需隱藏資訊，包括隱藏牌 ID、完整選項及序列化結算。

外部事件動態必須依觀看者過濾：

- 選擇玩家可看見允許其選擇的選項；
- 非選擇玩家只看見公開資訊，例如某位玩家正在選擇；
- 重播使用正規事件，絕不使用過濾後公開事件。

光芒（`光芒`）結算時也檢視下家手牌，須記錄當時手牌的正規快照。過濾後事件只有陣法施展玩家可看見快照；被檢視玩家、其他玩家及觀戰者只收到光芒已結算的公開事實。這是快照，不代表持續觀察後續手牌變化的權限。該快照須保留在施展玩家私密事件歷史中，使重新連線及重播不遺失已檢視資訊。

### 26. Game Over

來源依據：官方規則 7-3 在遊戲結束條件成立時立即結束遊戲，並停止尚未完成的效果處理。

實作解讀：只完成目前正在套用的同時語意結算，記錄明確終局結論，不執行後續效果或清理步驟。

隊伍生命值降至零或以下，或其他已啟用規則產生直接終局結果時，遊戲結束。儲存獨立遊戲結論：

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

依不變條件，`causes` 不可為空；具體 Rust 表示可使用非空集合型別。`GameEnded` 明確記錄最終結果與原因，不要求重播或呈現從先前傷害、勝利、陣法區或最近陣法事件推測。

雙人模式仍使用隊伍：各玩家屬於各自的一人隊伍。一隊生命值降至零或以下時，另一隊獲勝。

若同一次同時結算使所有對立隊伍生命值降至零或以下，結果為 `GameOutcome::Draw`。決定結論前先套用該同時語意結算中的每項變化，包括依規則在判定遊戲結果前發生的自動回應。

判定後立即發出 `GameEnded { conclusion }`，並使其成為最後正規事件。不開始後續依序效果、不僅為清理而捨棄陣法、不進入 `TurnDraw` 或 `TurnEnd`。已建立的牌區域保持不變，因此造成終局的目前陣法可留在玩家陣法區。

完成牌桌的呈現在依觀看者遮蔽事件後，另由包含 `GameEnded` 的已記錄決策推導。陣法成立或已結算攻擊可提供玩家、陣法及用牌；棄牌回收及自動效果各有呈現方式。此顯示是終局決策而非結束原因，也絕不遞補使用 `last_formation_by_player`。

到達 `GameStatus::Finished` 後拒絕遊戲命令。

生命值狀態下限為零，絕不儲存負值。

```rust
new_hp = max(0, old_hp - damage)
```

隊伍生命值上限為該局初始生命值。所有回復來源共用此上限，包括生陣、歸元、相生五行攻擊及經反震分攤的相生攻擊。

每個生命值變化事實均應保留足夠稽核／除錯細節，不論它巢狀位於 `AttackResolved` 事件，或由非攻擊來源發出：

```rust
struct HpChangeDelta {
    team: TeamId,
    old_hp: i32,
    delta: i32,
    new_hp: i32,
    effective_delta: i32,
}
```

`new_hp` 已受界限限制；`effective_delta` 記錄限制後實際狀態變化。

### 27. Initial Deal

引擎依已準備牌序發初始手牌並發出事件。

規則：

- 先手取得四張牌；
- 其他玩家各取得五張牌。

建議事件：

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

`GameSetup` 應定義玩家、隊伍、行動順序、生命值設定及規則集，不應直接包含起始手牌。

初始生命值由 `GameSetup` 提供，不由引擎硬編碼。

```rust
struct GameSetup {
    players: Vec<PlayerSetup>,
    turn_order: Vec<PlayerId>,
    hp: HashMap<TeamId, i32>,
    ruleset: RulesetId,
}
```

官方預設建構器可填入生命值：

```rust
GameSetupBuilder::official_2p(players)
GameSetupBuilder::official_team(players, teams)
```

測試及變體可提供明確生命值。

### 28. Team Setup Validation

即使輔助建構器產生合法設定，引擎仍必須驗證團隊模式設定。

驗證規則：

- 至少四位玩家；
- 恰好兩隊；
- 每位玩家均屬於一隊；
- 雙方玩家人數相同；
- `turn_order` 包含每位玩家恰好一次；
- `turn_order` 中相鄰玩家不可同隊；
- 檢查循環相鄰，因此最後及第一位玩家也不可同隊。

設定建構器可協助產生合法交錯座位，但仍必須拒絕無效設定資料。

### 29. Card Instances And Definitions

區域儲存實體牌，不儲存牌定義。

```rust
struct CardInstance {
    id: CardInstanceId,
    def_id: CardDefId,
}

struct GameSetup {
    card_instances: HashMap<CardInstanceId, CardDefId>,
}
```

牌堆、手牌、棄牌堆、陣法區、回合抽牌區及其他牌區域儲存 `CardInstanceId`。

規則及匹配器透過設定資料與牌定義登錄表解析 `CardInstanceId -> CardDefId -> CardDef`。

實體牌至定義的映射在遊戲設定後不可變；重播依賴穩定 `CardInstanceId` 值。

### 30. Card Definition Shape

第一版使用最小牌定義結構：

```rust
struct CardDef {
    id: CardDefId,
    name: String,
    element: Element,
    level: u32,
}
```

`level` 是穩定牌面屬性，供 `level + 4` 及 `sum(level) * N` 等基礎攻擊點數公式使用。

除非具體規則需要無屬性牌，否則不新增 `card_kind` 或選用屬性。

### 31. Base Formation Registry

第一版即建立 25 種基礎陣法的完整登錄表。

每個基礎陣法應有：

- `FormationDef`；
- `EffectDef`；
- 匹配器覆蓋。

效果解析器可逐步實作。已知但未實作的效果在完成前可回傳 `RuleError::EffectNotImplemented(effect_id)`，使驗證、schema 及匹配器測試能先於所有效果完成而穩定。

未實作效果不是驗證失敗。玩家對已知陣法提交合法命令，但其效果解析器尚未實作時，回傳實作錯誤，不發出事件或修改狀態。

```rust
enum CommandError {
    Validation(ValidationError),
    RuleImplementation(RuleImplementationError),
}
```

`EffectNotImplemented(effect_id)` 屬於 `RuleImplementationError`。

### 32. Status Duration

狀態持續時間使用明確到期時點：

```rust
enum Duration {
    UntilTurnStart { player: PlayerId },
    UntilTurnEnd { player: PlayerId },
    UntilTurnEndNumber { player: PlayerId, turn_number: u64 },
    Permanent,
}
```

第一版不包含 `UntilNextAction`。下次行動時的被動行為由玩家陣法區的蓋牌建模，不使用一般狀態持續時間。

核心模型避免通用 `remaining_turns`／`remaining_rounds` 計數器，因為它們在多人及團隊模式下有歧義。規則解析器可將規則文字轉為明確到期時點。

規則 6-1 與 6-2 依受影響者時點區分呈現單位：一回合在受影響玩家回合結束時經過，一輪在其回合開始時經過。因此 Web 呈現只計算該玩家對應邊界，不得以到期序號減目前全域 `turn_number`，並將差值標成剩餘回合。`UntilTurnStart` 狀態以輪顯示持續時間；天陽罡因公開持續時間為一輪而使用此時點。

狀態到期記入事件日誌：

```rust
GameEvent::StatusExpired {
    status_id: StatusId,
    owner: StatusOwner,
    expired_at: ExpiryTiming,
}
```

`advance_until_decision` 在 `TurnStart` 及 `TurnEnd` 時點檢查到期狀態；已到期狀態透過事件移除，不靜默修改狀態。

### 33. Replay Versus Verification

純重播只套用已記錄事件，不重新執行命令決策邏輯，也不重新計算自動事件。

模式：

- 即時執行：命令及自動推進決定事件後套用；
- 重播：完全依記錄套用正規事件日誌；
- 驗證：可選擇重新執行命令／自動推進，並比較產生事件與正規事件日誌。

正規事件日誌已包含初始發牌、洗棄牌、選擇請求及狀態到期等自動事件。重播時重新計算可能重複事件，或因規則集版本不同而產生分歧。

正規 `GameEvent` 及 `PendingChoice` payload 參與重播驗證。僅客戶端需要的資料，例如效果選擇要求牌數或終局決策顯示，屬於 Web 投影，並由正規紀錄及公開狀態推導。

### 34. Event Granularity

來源依據：官方規則 2-5.3c 及 5-2.4i 使攻擊傷害與所有未指定其他時點的附帶效果同時且無順序。

實作解讀：以一個原子 `AttackResolved` 事件表示完整攻擊結算。即使一系列事件各自有語意，仍會強加規則明確否認的正規先後關係。

使用包含明確可重播變化的語意事件。避免過於模糊、重播時須重新計算規則的事件：

```rust
GameEvent::FormationPerformed { formation_id: FormationId }
```

不新增抽象 `FormationUseCompleted` 事件。正常完成由下一個具規則意義的事實表示，即 `FormationCardsDiscarded` 或 `PassiveCovered`；終局陣法則以 `GameEnded` 結束。

也應避免整份日誌只有沒有領域意義的低層修改事件：

```rust
GameEvent::CardMoved { ... }
GameEvent::HpChanged { ... }
GameEvent::StatusAdded { ... }
```

優先使用帶具體變化的語意事件：

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

這使日誌容易理解，並允許 `apply_event` 直接從記錄事實修改狀態，而非重新計算規則結果。陣法牌基本移動由 `FormationCommitted` 及 `FormationCardsDiscarded` 表示，不在每個效果事件內重複。

`AttackDamageResolution` 包含攻擊傷害造成的所有生命值及防護罩結果，包括分攤或反彈份額。`ResolvedAttackEffects` 包含相同時點附帶效果的所有可重播變化，包括適用時額外隊伍生命值或防護罩變化、狀態施加、牌或環境變化，及回合抽牌加成。套用 `AttackResolved` 時原子套用完整 `AttackResolution`。

`AttackResolution` 的欄位排列及其中集合只使用穩定正規序列化順序，位置絕不代表規則處理順序。

屬於同次攻擊結算的結果不得分別發出 `HpChanged`、`ShieldChanged`、`StatusAdded`、`CardsMoved` 或模組專屬效果事件。這些事件仍可用於非攻擊來源及規則明確指定其他時點的攻擊效果。

附帶效果需要玩家輸入時，先記錄其待選擇事項及續接，不套用傷害或其他同時結果。最後答案後才發出完整 `AttackResolved`；其後判定遊戲結果，因此致命傷害變化不會抑制同次攻擊結算的其他結果。

`TurnDrawResolved` 明確記錄所選棄牌及所有保留牌。套用此單一事件時，先將棄牌從回合抽牌區移至適用來源棄牌堆，再將保留牌移入手牌；中間不存在正規狀態。

單一命令或自動推進可發出多個語意事件。例如攻擊結算可能發出：

```rust
GameEvent::FormationCommitted { ... }
GameEvent::PassiveFlipped { ... }
GameEvent::AttackResolved { ... }
GameEvent::FormationCardsDiscarded { ... }
GameEvent::CardsDrawnForTurnDiscardChoice { ... }
```

`AttackResolved` 產生終局結論時，序列緊接著發出 `GameEnded { conclusion }` 並停止，不發出 `FormationCardsDiscarded`、`CardsDrawnForTurnDiscardChoice` 或其他後續事件。

不強制將所有結果放進一個大型 `CommandResolved` 事件。事件邊界應遵循有意義的領域時刻，尤其是結算可能因待選擇事項暫停之處。

以已記錄事件中繼資料包裝領域事件：

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

`GameEvent` 持續聚焦領域事實；`RecordedEvent` 攜帶供除錯、介面確認、網路及稽核使用的順序與來源中繼資料。

重播依賴 `sequence` 與 `event`，不依賴 `command_id`。不在 `GameRecord` 中持久化獨立命令日誌。

`RecordedEvent.source` 足以關聯已接受命令及其發出的事件。驗證失敗是 API 錯誤，不屬於正規遊戲紀錄。

### 35. Error Taxonomy

使用三個命令錯誤層：

```rust
enum CommandError {
    Validation(ValidationError),
    RuleImplementation(RuleImplementationError),
    EngineInvariant(EngineInvariantError),
}
```

意義：

- `Validation`：玩家命令不合法，可向介面顯示；
- `RuleImplementation`：規則模組或效果實作不完整／錯誤；
- `EngineInvariant`：到達不可能狀態或違反不變條件。

`EngineInvariant` 範例：

- 牌堆加適用棄牌堆不足以滿足必要抽牌；
- 重複待選擇事項；
- 回合流程本應阻止重複蓋牌的狀態下出現重複蓋牌；
- 區域擁有權不一致。

命令及自動推進錯誤均為原子。`decide` 因任何 `CommandError` 失敗時，不發出事件，也不修改狀態。

`handle_command` 只在完整事件清單成功決定後才套用事件。

`apply_event` 是正規事件上的純投影，信任事件日誌，不重新執行遊戲驗證或不變條件檢查。驗證及不變條件檢查在事件記錄前或明確驗證工具中進行。

由於 `apply_event` 信任正規事件，應為不可失敗：

```rust
fn apply_event(state: &mut GameState, event: &GameEvent);
```

### 36. Five Directions Legend Rule Module

將官方[五方傳說規則](https://www.cfecards.org/rule/latest/field)實作為一個可獨立設定的進階規則模組。啟用後加入全部五方聖獸、一個共享環境及虛空斷脈術；這些功能均沒有獨立開關。

遊戲開始時沒有環境，環境至多持有一個 `Element`。每次聖獸攻擊結算後均發出具語意的正規 `EnvironmentTransferred` 事件，包括轉移前後屬性相同時。攻擊使用先前存在的環境；致命攻擊在陣法施展與遊戲結果最終確定前，仍須完成環境轉移。

每個聖獸均為五張同屬性牌組成、81 點的五行攻擊：

- 東‧青龍：木；
- 西‧白虎：金；
- 南‧朱雀：火；
- 北‧玄武：水；
- 中‧黃蛇：土。

聖獸不受陣法效果影響。蓋牌及公開反制效果仍在正常時點觸發並消耗，但對其結算為 `NoEffect`。一般防護罩規則仍適用，因為防護罩是獨立遊戲要素，而非傳入陣法效果。

環境效果適用於所有玩家：

- 傷害屬性與環境相同時傷害加倍，並與五行生剋疊加；
- 傷害屬性生環境屬性時轉為回復生命；
- 同時相剋使該回復加倍，相抵則使其減半並無條件進位；
- 攻擊明細分別記錄環境效果、`ElementInteraction` 及最終數值。

防護罩先於擁有者承受傳入攻擊；由防護罩承受時不適用五行生剋。由於防護罩沒有生命值，將傷害轉為回復生命的環境效果因此不適用；但同屬性環境加倍仍改變防護罩承受的傷害。

每種環境使兩個具名陣法無效：

- 金：防禦及氣壁；
- 木：幻化及混沌；
- 水：反震及震暴；
- 火：武器及光芒；
- 土：封印及歸元。

無效陣法仍可合法施展、消耗行動機會並遵循正常牌移動程序，但效果結算為 `NoEffect`。攻擊及主動術式以目前環境為原因發出 `FormationEffectIgnored`；蓋牌保留施展時確立的無效狀態，並在日後翻開結果中回報，不重新判定已改變的環境。已確認時點及目前實作缺口見第 45 節。無效判定檢查實際施展陣法身分，不檢查幻化複製的效果。既有持續狀態不被移除；例如無效氣壁不取代或清除現有防護罩。

虛空斷脈術是三張同級牌組成的主動術式。沒有環境時正常施展但不改變生命值。有環境且術式未因其他原因無效時，破除該環境並使每隊生命值恰好減少 20，與玩家人數無關；防護罩數值不影響。以一個 `EnvironmentCleared` 事件記錄破除環境及所有隊伍生命值變化，套用全部變化後才判定遊戲結果，使同時敗北成為平局。

官方設定生命值遵循目前規則書，不使用先前 20 點佔位值：

- 僅基礎規則的雙人及四人遊戲，隊伍生命值分別為 100 及 150；
- 使用進階規則模組的遊戲，分別為 200 及 250。

模組設定、目前環境、正規事件、攻擊明細及遊戲結果必須確定性重播，並投影至公開狀態視圖、公開事件動態、Web DTO 及線上戰場。

### 37. Hero Schools Rule Module

將官方[英雄學派規則](https://www.cfecards.org/rule/latest/hero)實作為一個可獨立設定的進階規則模組，包含所有職業、職業能力、職業陣法、轉職及虛空返璞術。新官方遊戲預設啟用完整模組；Web 房間設定只提供一個英雄學派開關。

每位玩家開始時沒有職業。包括初行客在內，職業只能透過成功轉職取得。

轉職是獨立行動命令，不是陣法施展。它驗證宣告職業、先決職業及提交實體牌，接著進入 `Action` 並處理上家蓋牌。處理先前蓋牌期間，所選牌在正規狀態中仍留在手牌。轉職繼續時，一個語意 `ProfessionChanged` 事件以原子方式同時記錄新職業，並將這些牌直接從手牌移至適用來源棄牌堆；沒有暫存轉職牌區。命令消耗目前玩家行動。不得將轉職表示為 `PerformFormation`，也不得與幻化混淆。

遊戲狀態只儲存每位玩家目前的 `ProfessionId` 或無職業，不複製職業有效能力。`ProfessionChanged` 記錄前後職業身分。官方職業目錄透過五大學派明確繼承連結推導有效能力；仙者及聖者沒有繼承連結，轉為任一職業時移除前一職業能力。

所有職業發動能力使用同一個 `ActivateProfessionAbility` 主動效果命令，以及每位玩家回合共用的一次發動次數。成功發動消耗次數但不消耗行動機會；驗證失敗兩者均不消耗。轉職不重設次數，因此同回合取得不同發動能力絕不允許第二次發動。

`rules::profession::activated` 是這些命令的單一生命週期擁有者，對基礎規則的介面只有選項產生與發動。它將外部字串 ID 解析一次為巢狀封閉 enum，委派給靜態分派的英雄學派、江湖、匯流世代或黑暗微光提供者；未知 ID 直接拒絕，不使用前綴遞補或執行期註冊。共用模組依序驗證有效職業能力集合、暫時失去能力、共用回合次數，以及私有且已驗證的行動選牌。

各提供者回傳包含一個完成契約及宣告式玩家中繼資料的選項計畫；命令驗證重新建立相同選項權威。目標牌、宣告屬性及宣告等級各自必須不存在、等於選項固定值，或恰屬提供的必要值；無關欄位及遞補式省略均為驗證失敗。因此暗靈只接受所選牌提供的有效屬性，不能透過直接命令改變該屬性。

效果規劃使用單一暫用投影。共用建構器先恰好記錄一個 `ProfessionAbilityActivated` 事件，再依序記錄提供者結果，最後至多有一個媒介符合其 `PendingResolution` 的選擇或隨機性續接。提供者不能自行記錄發動事件。這使代價可在規劃牌堆供給前進入來源棄牌堆；因此啟示及青雲步會將剛捨棄的代價牌納入必要洗棄牌。

青雲步使用兩個具型別續接。牌堆不足時等待僅隨機性的 `JianghuAzureCloudStepDraw`；洗棄牌後抽兩張牌，並建立既有僅選擇的 `JianghuAzureCloudStepReturnOne`。

專精能力為現有陣法增加玩家範圍的替代匹配器，不註冊重複陣法，也不取代原陣法定義。透過專精匹配的陣法施展保留原陣法身分、類別、效果、點數公式、牌移動及正規事件語意。只有施展玩家目前職業授予該匹配器時才可用；公開陣法查詢可將匹配專精標作呈現中繼資料。

同一組提交實體牌若有多種合法角色分配且會改變結果，陣法匹配回傳不同 `FormationMatchOption`。`PerformFormation` 必須宣告其中一個選項，不讓引擎最大化或自行選擇結果。已接受陣法事件記錄結算後角色綁定與計算；查詢公開合法選項及結果預覽。只有一個無歧義選項時不需額外 Web 互動。

虛空返璞術以一個原子語意 `VoidReversionResolved` 事件結算，記錄施展玩家隊伍生命值減少 20、所有實際被破除職業、因用牌低於三級而保留的傳說職業，以及陣法牌移動。判定遊戲結果前套用所有變化；代價使生命值歸零不會中止破除職業或暴露中間狀態。

普通能力及專精能力透過明確規則階段的具型別模組鉤子整合。鉤子可擴充陣法匹配、修改代價或攻擊計算、判定免疫、修改反制效果適用性或回傳陣法後意圖；它們為共用流程回傳宣告值，不直接修改遊戲狀態或發出事件。不得將職業能力編碼為一般狀態效果，也不得在基礎解析器中散布職業身分檢查。發動能力提供者則為玩家新選擇的主動效果回傳效果計畫；它們不是階段鉤子，也不繞過共用發動生命週期。

跨模組攻擊結算使用明確階段順序：

1. 套用合法牌解讀並選擇陣法匹配選項。
2. 計算攻擊點數，包括職業點數修正。
3. 判定風行術及召喚星辰等攻擊點數資格。
4. 結算適用反制效果。
5. 套用五行生剋、環境效果及防護罩，得到最終傷害或回復結果。
6. 結算陣法後職業意圖。

因此風行術 15 點門檻讀取環境傷害修正前的攻擊點數。正規攻擊事件分別記錄攻擊點數與最終結果。

實體牌準備與虛擬陣法組件是不同領域概念。烈陽訣與暗靈為一張實體牌建立可序列化的實體牌解讀；幻術與幻朧則捨棄兩張實體牌，建立來源能力、屬性及等級固定的 `VirtualFormationCard`，不指定或重新解讀第三張實體牌。

實體準備的能力身分及解讀後事實公開，但目標實體牌在正常牌移動使其公開前只對擁有者可見。幻術與幻朧的代價牌是一般公開棄牌；產生的虛擬屬性及等級在能力結算後立即公開。

虛擬牌立即公開、不屬於任何牌區域，且沒有實體牌或牌來源。牌解讀層、錦囊加成及星辰變牌只適用實體牌，不能改變虛擬組件。陣法匹配、預覽計算、點數公式、效果結算及計算陣法組件的規則，均使用同一個已結算 `FormationComposition`；牌移動、手牌數、覆蓋及要求實體牌的效果只使用其中實體牌。

幻術建立 `FormationRequirement`，唯一允許的完成方式是對應基礎規則五行擊術。幻朧建立相同種類要求，但允許任意基礎規則陣法；單組件五行擊術使每個合法屬性及等級宣告均有完成路徑。伺服器只在陣法查詢與提交時自動加入虛擬組件。玩家選牌集合仍是實體牌，因此不結束行動的精靈技能及其他能力不會將虛擬組件視為已選牌。

發動前，可執行行動查詢對幻術或幻朧各公開一個選項，附帶包含合法虛擬牌屬性及等級的具型別行動輸入要求，不為每個屬性／等級組合枚舉完整選項。選擇這些輸入仍是提交前瀏覽器狀態；只有已接受的 `ActivateProfessionAbility` 命令才建立虛擬牌、消耗發動次數並確立陣法要求。此提交前輸入不是待選擇事項，也不在重新連線後保留。

暗靈為所選牌同時建立實體等級解讀與陣法要求。只有一級或二級值低於該牌目前有效等級時才能宣告；伺服器在陣法查詢及提交中自動將必要牌恰好納入一次。略過、轉職及無法滿足有效陣法要求的陣法均不合法。不結束行動的效果只有在保留完成路徑時才能先發生。無效提交使要求繼續有效；已接受的允許陣法即使之後被取消、防止、封印或無效，仍滿足要求。

烈陽訣將所選木或火牌提高兩級，上限五級，不建立陣法要求。玩家該回合若使用該牌，提高後等級是唯一可用等級；它只能參與基礎規則陣法，不能用於轉職或其他規則模組陣法。玩家也可選擇不使用準備牌的行動。

實體牌解讀層依維度及效果順序組合。因此錦囊、火精靈、烈陽訣、暗靈及星辰效果為每個實體組件解析單一有效屬性與等級。陣法匹配不建立分離的牌面與準備候選；已接受陣法結算不遞補使用牌面或全域快取牌資訊。正規陣法要求完成事件記錄完整陣法組成，使重播及稽核不從先前發動事件推測虛擬事實。

Web 不為虛擬牌虛構牌面呈現、狀態徽章或陣法名稱註記。公開事件動態記錄其建立及要求完成。幻朧覆蓋被動術式時，陣法身分及實體牌保留既有蓋牌遮蔽，而已公開虛擬事實仍公開。

陣法匹配區分實體牌與匹配位置。聖術可將一張實體牌展開成兩個陣法匹配位置，但牌移動與一般等級總和公式只計算該牌一次。`FormationMatchOption` 將兩個位置都記錄為源自同一實體牌。Web 只以 `×2` 匹配註記呈現，棄牌數及點數預覽仍依實體牌計算。

聖術視為兩張在實體牌解讀層解析後套用，不是另一種屬性或等級轉換，且仍不相容星辰變牌。重複匹配位置共用同一實體牌的有效事實；牌移動及一般實體牌數仍只計一次。

正常 Web 互動仍先選牌。既有可施展陣法查詢深化為可執行行動查詢，同時回傳恰好匹配所選實體牌的陣法及轉職候選；它不枚舉所有可能手牌組合，也不替玩家選牌。獨立教學模式投影可顯示完整職業進階圖、條件及能力變化。

戰場依情境調整的陣法控制介面保持**能力**與**行動**可區分。**能力**群組包含主動效果命令並說明它們不進入 `Action`；**行動**群組包含陣法施展、轉職及行動略過，並說明選擇後進入 `Action`。呈現遵循[戰場介面設計](../apps/web/docs/ui-battlefield-design.md)，取代先前要求所有視窗尺寸永久垂直堆疊兩個面板的要求。不新增獨立職業面板。普通能力及專精能力透過合法選項與說明呈現，不作為控制。

目前有職業的每個玩家席位顯示公開職業徽章。選取徽章開啟該玩家有效能力的唯讀摘要；發動控制仍位於中央能力面板。沒有職業的玩家不顯示徽章。

英雄學派可依房間所選規則版本及模組設定啟用。等待室房主變更規則模組時，準備狀態及鎖定設定輸入失效。

英雄學派可在內部分垂直切片實作：共用職業基礎、五個學派各一切片、無學派職業加虛空返璞術，最後進行跨模組、Web、教學及 E2E 整合。未完成模組不加入可用官方目錄；只有完整 18 個職業及端到端行為才透過單一英雄學派房間開關發布。

驗證結合窮舉規則案例與有界整合覆蓋：

- Rust 符合性測試涵蓋所有轉職、繼承能力、普通能力、專精能力、發動能力及職業陣法。
- 兩兩整合測試分別結合英雄學派與星辰圖記、五方傳說、棄牌回收及個人牌組。
- 至少一個雙人及一個四人瀏覽器流程啟用所有可用模組。
- 僅基礎規則及停用英雄學派套件維持回歸閘門。

不要求所有規則模組設定的完整笛卡兒積。

英雄學派符合性固定於 2026-07-01 取得的官方 5.16 規則。所連官方頁面仍為來源參考，但其可變 `latest` 內容變更不會在交付中途改變本實作驗收條件。後續官方修訂需要獨立規則升級工作；本變更不引入通用規則模組版本系統。

### 38. Official 5.16 Advanced-Rule Source Reconciliation

2026-07-01 已將三個官方進階規則網頁及六個英雄學派子頁，與[官方 5.16 完整規則書](https://www.dropbox.com/scl/fi/4cigz3t61p07l5tkgvjyl/5.16.pdf?dl=0&rlkey=m2zx1x5dwb6cw7llyy8wt9xiy&st=mqligrpj)印刷頁 13–24 比較。網頁仍是一般可閱讀來源。下列僅 PDF 有的細節會改變匹配、時點或結算，因此為具規範性的補充；僅重述規則或增加不產生區別之範例的差異不列出。

#### Star Chart

相較[星辰圖記網頁](https://www.cfecards.org/rule/latest/star)，印刷頁 13 增加：

- 除非該星辰已存在或其他規則使召喚不可能，符合條件的星辰召喚為強制。
- 星辰變牌只在判定陣法組成時改變牌屬性；其他時間該牌維持牌面屬性。
- 以變牌覆蓋被動術式時，玩家必須在覆蓋時宣告變牌並指出受影響的反面牌；該牌不翻開。

網頁已說明變牌只影響一張牌且只適用基礎規則陣法，因此不能用於進階或主題陣法，也不能用於轉職。

#### Five Directions Legend

相較[五方傳說網頁](https://www.cfecards.org/rule/latest/field)，印刷頁 15 增加：

- 聖獸結算傷害後才改變環境，因此其自身傷害受先前環境修正。
- 不受陣法效果影響不代表不受非陣法規則影響；職業抗性仍適用於聖獸傷害。
- 環境傷害加倍發生於五行相生、相剋或相抵之前。
- 環境轉換與五行相生都會使同一傷害轉為回復時，只回復一次而非兩次。
- 虛空斷脈術只有實際破除環境時才扣除生命；沒有環境或其他效果阻止破除時不扣除生命。

#### Hero Schools: General Rules

相較[英雄學派網頁](https://www.cfecards.org/rule/latest/hero)及其學派子頁，印刷頁 17–18 增加：

- 轉職依序宣告職業、展示必要牌、翻開並結算上家蓋牌、轉職，最後捨棄必要牌。
- 轉職是行動但不是陣法；適用陣法施展的效果不適用轉職。
- 普通能力不能拒絕。玩家無法行動或能力完整效果無法達成時不能發動能力。
- 無學派職業不構成同一學派；即使兩名玩家持有同一無學派職業，也不算同學派。
- 屬性抗性使傷害無效，因此該傷害的五行相生不回復生命。

戰士網頁已包含 PDF 的防護罩釐清：抗性及卸勁只在玩家承受傷害時適用，因此防護罩承受未減少傷害。

#### Hero Schools: School-Specific Rules

- **戰士：**印刷頁 19 與[戰士網頁](https://www.cfecards.org/rule/latest/hero/warrior)對同時覆蓋木與火時如何依實體牌位置區分防禦及反震有所衝突。PDF 在牌不重疊時優先左牌，無法判斷左右時才用較近玩家的牌；網頁優先較近的牌，無法判斷遠近時才用玩家左側。線上引擎不推測實體排列；兩者均合法時要求覆蓋玩家選擇防禦或反震，並記錄選擇。
- **尋道者：**印刷頁 20 新增[尋道者網頁](https://www.cfecards.org/rule/latest/hero/seeker)沒有的一個具區別性轉生術範例：木 2、木 3、火 3、土 4 中，只有木 2 可作為獨立木牌，因為木 2 加火 3、土 4 總和只有 9。只有一個合法選項，回復 50 點生命；只有多種角色分配均合法時才需選擇。
- **幻術師：**印刷頁 21 將幻術及幻朧結果定義為捨棄兩張手牌後產生的 `虛擬牌`，而非重新解讀第三張實體手牌。它也明確指出幻朧與幻術是不同發動能力，因此幻朧不觸發幻術精研（`幻術精研`）。這與[幻術師網頁](https://www.cfecards.org/rule/latest/hero/mesmer)一致，但網頁未直接說明。
- **風行者：**印刷頁 23 將刃影瞬殺生命減半分類為扣除生命。結算時將目標隊伍生命值設為 `floor(original HP / 2)`，並記錄所得負生命值變化。[風行者網頁](https://www.cfecards.org/rule/latest/hero/windwalker)說明無條件捨去，但未分類為扣除生命。
- **法師及無學派：**一般英雄學派頁面與各自的[法師](https://www.cfecards.org/rule/latest/hero/mage)及[無學派](https://www.cfecards.org/rule/latest/hero/others)子頁合併後，沒有額外語意規則。

### 39. Theme Rule Module Delivery Scope And Defaults

下方全域目錄預設政策已由 [ADR-0038](adr/0038-bind-games-to-rule-versions.md) 取代：可用主題由所選規則版本決定，與遊戲啟用的模組子集分開。本節其餘執行決策繼續適用。

將江湖、匯流世代及黑暗微光加入為三個可獨立選用的主題規則模組，依此順序交付。每次交付均是完整垂直切片，貫穿 Rust 規則引擎、正規重播、依觀看者過濾的投影、線上房間設定、介面及自動化測試。

規範來源為提供的 `cfecards-5.16.pdf`。實作這三個主題模組的完整規則，以及它們與基礎規則集、三個進階規則模組及精靈規則模組的互動。對延時術式等未實作主題模組的引用保持不生效，直到那些模組另行加入；不得虛構佔位行為。

每個主題規則模組均可獨立選用，並可與其他模組組合。與精靈一樣，江湖及匯流世代依賴星辰圖記、五方傳說及英雄學派；黑暗微光的惡精靈與死精靈使用精靈規則，因此另依賴精靈。選用主題模組自動選用傳遞依賴；移除依賴會移除所有依賴它的主題模組。

新遊戲及線上房間預設啟用所有可用規則模組，包括精靈、江湖、匯流世代及黑暗微光。已持久化房間保留明確儲存的模組清單，不隱含取得新發布模組。

錦囊發布後遵循相同預設，即使它為遊戲準備增加初始錦囊選擇。既有房間保留儲存模組清單；只有新建房間預設啟用錦囊。

#### Pouch execution model

牽羊以一個命令攜帶兩組選牌，但兩組不共用單一驗證快照。先對效果前個人牌組驗證第一組，投影依序由牌堆至棄牌堆的移動，再對施展玩家投影後棄牌堆驗證放回組。只有來源屬於該玩家的實體牌，才能在一次結算中由牌堆移至棄牌堆再回牌堆；公開外來牌回到來源擁有者的堆，不是可放回候選。正規 `CardsMoved` 事件依結算順序記錄這些變化，重播依序套用。

私密互動選項同時公開目前棄牌堆，以及第一步選取後會進入該玩家棄牌堆的個人牌組牌。只有玩家選該預期牌堆牌作為棄牌後，介面才將其呈現為放回選項。這是有序效果的互動投影，不是新的正規待選擇事項。

連環不帶擁有者的 `PouchRevealed` 事件將觸發來源牌從其玩家牌堆置於一旁；所選錦囊牌在同一事件批次由 `PouchPlaced` 移除。連環接著投影所選秘計，再請求檢索後牌堆洗牌；後續 `PouchConsumed` 事件將置於一旁的來源牌移至其來源棄牌堆。因此洗牌記錄的 `currentOrder` 不含兩張所選牌，並在可信任隨機性結算時仍與牌堆相符。牽羊是連環額外洗牌請求的例外：重用其交換後牌堆洗牌，因此連環不發出重複洗牌。

離山刻意以**暫時失去能力**取代官方 5.16 的效果無效解讀。官方規則 4-7 廣泛的「該職業之能力」邊界具權威性：受影響玩家保留職業身分，但暫時失去完整職業能力集合，包括普通、專精及發動能力、職業陣法、繼承規則及其他職業規則。精靈範圍同樣移除主動、自動及常駐精靈技能，並持續禁止增加靈力。見 [ADR-0033](adr/0033-treat-lure-as-temporary-ability-loss.md)。

可執行行動產生及命令驗證共用目前能力的權威來源。需要已失去職業能力集合或精靈技能的選項不提供；過期或直接提交屬驗證失敗，不發出正規事件或產生代價。途徑不依賴已失去集合時，一般轉職仍合法：初行客授予的抉擇及突破許可不存在，保留的職業身分仍可能滿足一般先決條件，已失去的暗行禁止不再阻止一般轉職。職業陣法及職業授予的轉換或取得結果在持續期間仍不可用。

暫時失去能力只影響未來，不溯及既往。已接受能力使用保留其已準備職業能力、陣法要求、牌解讀層、加成、狀態效果、代價及已消耗次數；套用或離山到期均不回滾或重設這些事實。失去期間自動檢查不使用缺失能力，包括綻放與順風次數回復。轉職、召喚精靈及其他來源替換均不能避開此玩家範圍持續時間。

既有職業範圍及精靈範圍離山狀態效果仍是正規及公開狀態表示；事件或待選擇結構不變。金蟬仍只保護面向玩家的職業範圍，不保護精靈技能或靈力增加。公開呈現保留所持職業、精靈及唯讀摘要，依賴既有可見離山狀態，且只從可執行行動推導可操作控制。

江湖術語**狀態（State）**比引擎既有一般 `StatusEffect` 概念狹窄。定義只包含千鋒、踏雪及中毒的獨立具型別江湖狀態集合；不重新命名或改變一般模型及序列化 `statuses` 契約。新增正規及公開狀態欄位使用向後相容預設值。見 ADR 0012。

移除既有職業教學／目錄對話框，不為新主題職業擴充它。戰場保留各玩家目前職業徽章及唯讀有效能力摘要。匯流世代公開次數限制顯示在授予該次數的職業或能力旁，因為它們是必要目前遊戲狀態，不是教學內容。

所有已啟用規則模組共同提供一個組合職業目錄及每玩家一個職業欄位。即使職業明確繼承另一模組定義職業的能力，各模組擁有的定義與具型別鉤子仍分離。見 ADR 0013。

江湖中毒在結構上只有一個來源：所有造成中毒的陣法均影響施展者下家，而固定行動順序使每個中毒玩家恰有一位上家。因此重複施加中毒只增加單一剩餘回合數，不引入各來源分段。每次中毒回合結束時，毒絕讀取唯一來源玩家當時的職業，也涵蓋該玩家轉為毒聖前已施加的中毒。

來源術語 `星行牌` 與 `環行牌` 均讀取牌面屬性，不讀臨時牌解讀。星行牌符合相關玩家隊伍目前擁有星辰的屬性；環行牌符合規則檢查時的目前環境。相應星辰或環境不存在時沒有牌符合。

匯流世代的 `上家棄牌` 是既有可回收棄牌：上家剛結束上回合的回合抽牌棄牌，前提是同一實體牌仍在任意棄牌堆；目前堆所有者不影響回收。移至牌堆、手牌或其他非棄牌區會使實體牌不可回收。相對地，餘行（`餘行`）與餘級（`餘級`）在該牌被捨棄時依牌面定義固定，直到目前玩家進入回合抽牌前均不變，即使牌已移動；移動只使調律或天響不能回收該實體牌。

黑暗微光戲鬧（`戲鬧`）依觸發效果實際檢視實體牌中的最高牌面等級計算扣除生命。因此惡視只使用隨機檢視的兩張牌，檢視完整手牌的效果則使用整手牌。正規事件記錄檢視集合，但觀看者過濾僅向檢視玩家揭露，絕不補入未檢視隱藏牌。

晴風使的順風只在洗棄牌完成時回復次數；重新排列已在牌堆中的牌絕不回復。共用牌堆洗棄牌時，每位目前擁有順風且職業能力集合可用的玩家均回復次數；啟用個人牌組時只有被洗棄牌的棄牌堆擁有者可回復。只有實際從零增加至目前上限時記錄明確 `LimitedUseChanged`；已達上限、職業能力集合不可用或已不再擁有順風的玩家不產生回復事件。

本機房間 `44e4b60f-89c9-4e7e-a832-521af4a5aa3c` 是修正此區別的回歸情境：命令 39 消耗 player-2 的順風，命令 72 對 player-2 個人牌組施展鏽鐵枯林。其可信任洗牌屬牌堆洗牌，因此順風必須維持已用盡。

死精靈同命（`同命`）**不**從已結算陣法的受影響玩家集合推導。已確認規則是：陣法效果實際扣除死精靈擁有者隊伍生命時，該擁有者下家的隊伍扣除 10 點生命。因此此隊伍生命值模型中相關事實是有效隊伍生命值變化。`AttackResolved` 事件的基本攻擊傷害不符合；陣法獨立扣除生命效果在免疫或防止後確定有效變化時則符合，即使原子事件序列化將該獨立效果與攻擊結果並列。同命附加在符合條件的扣除之後，將多位擁有者若共享目標隊伍的結果合併為單一生命值變化，且不能遞迴觸發另一個同命。

虛空碎靈術使死精靈失去兩點靈力後若仍被擁有，只有該術陣法效果扣除其擁有者隊伍生命時才觸發同命。被該術減至零並破除的死精靈不觸發；魔靈復甦新召喚的精靈在觸發扣除時尚不存在，也不追溯觸發。

下家適用牌堆有一至三張牌且棄牌堆非空時，死兆（`死兆`）使用既有可信任洗棄牌邊界。其 `RandomnessRequested` 包含具型別精靈 `deathOmen` 續接，是唯一初始正規事實：它不消耗技能每回合次數或靈力、不移牌、不改變生命值，也不破除精靈。`RandomnessResolved` 將完整洗後棄牌堆移至該牌堆底後，續接發出一般技能使用及結算事實。因此重播、線上房間正規紀錄及公開視圖看到相同等待狀態與記錄洗牌順序。

#### Jianghu execution model

正規遊戲狀態將江湖狀態與一般狀態效果分開儲存。千鋒及踏雪保留持續至擁有者下回合結束的明確到期時間，並立即適用於建立它們的陣法。中毒在受影響玩家身上儲存單一合併剩餘回合數，在該玩家回合結束時扣除生命並減少回合數，且只阻止陣法提供的生命回復。江湖狀態的施加、延長、減少及結束均記錄為事件。

具型別江湖鉤子參與公開規則要求的階段：陣法匹配、攻擊點數計算、陣法效果免疫、回復結算、陣法後結果及回合結束。它們回傳宣告式變化，不直接修改遊戲狀態。

#### Confluence Generation execution model

次數限制是公開正規紀錄，以玩家及穩定陣法或能力身分為鍵，明確記錄上限及剩餘次數。消耗、回復、上限改變及重新取得職業時重設均為語意事件。由道法師轉為道法聖時，禁錮法陣上限與剩餘次數各加一，不重設先前使用；無盡法陣使其回復至目前上限。

調律建立回合內、特定牌的使用義務：回收牌必須參與該回合轉職，或在易弦適用時參與允許的職業陣法。只有至少一個此類行動可完成時，發動調律才合法；天響回收同一張牌則不受此限制。檢視牌堆、保留牌、選擇五鳴術或決定晴風展示牌頂的選擇，使用具型別待選擇事項與可重播續接。

調律合法性檢查依可達性，而非能力使用後立即檢查。它包含剩餘不結束行動的能力，只允許投影結果保留至少一條合法完成路徑的中間選擇。規則引擎對行動查詢與命令驗證均套用此規則。調律牌參與已接受的允許行動即完成義務，即使該行動效果之後無效或被取消；其他自願效果均不得消耗該牌。

調律師系統餘行轉職總和 3、6、9 是最低總和。餘級是牌面基準；提交牌及調律棄牌代價比較經適用解讀後的有效等級。除非規則明確允許一牌視為多張，每個陣法角色均需不同實體牌，因此五鳴陣法的屬性牌及餘級牌角色不能由同一實體牌填入。易弦許可涵蓋所有目前可用職業陣法，包括繼承的調律師陣法。

千鳴通常同時結算兩個鳴術。鏡鳴需要選擇時，先完整結算餘行鳴術續接，再結算指定鳴術；先前鳴術致命也不截斷後續鳴術，遊戲結果等待完整陣法結算。萬鳴則先套用確定性的生命值、防護罩及回合抽牌變化，再進入鏡鳴選擇；最終遊戲結果仍等選擇完成後才判定。

煌鳴及包含煌鳴的複合鳴術只影響上家；其隊友共享隊伍生命值變化，但不加入受影響玩家集合。森鳴及萬鳴回復施展玩家隊伍生命值，但中毒或其他施展者範圍回復限制只檢查該施展者，不檢查隊友。

#### Dark Glimmer execution model

惡精靈與死精靈擴充玩家既有單一精靈欄位，並保留零至六靈力規則。魔靈附體改變目前精靈種類而不改變靈力；魔靈復甦及兩個召喚陣法以公開初始靈力召喚新精靈。常駐精靈技能透過具型別鉤子執行，絕不消耗每精靈每回合一次的發動技能次數。

暗行是共用的一般轉職許可條件，可執行行動產生及命令驗證均使用它，因此暗行者及繼承其能力的暗靈使不提供也不接受一般轉職。暗行者 → 暗靈使自動轉換是暗黑陣法結果，不在一般命令條件內。

隨機選取及檢視手牌使用可信任應用程式隨機性邊界；正規事件記錄所選實體牌及順序，公開視圖保留規則的檢視邊界。暗黑陣法的環境無效依實際施展陣法身分判定。暗境只為其擁有者繞過此無效；英雄學派影遁鉤子只忽略另外具名的兩個暗黑陣法效果。

### 40. Tribulation Theme Rule Module Scope And Source Interpretations

將天劫加入為可獨立選用的主題規則模組，並以完整垂直切片交付 Rust 規則引擎、正規重播、依觀看者過濾的投影、線上房間設定、介面及自動化測試。它遵循既有主題規則模組依賴與預設選擇政策。

五種天劫均為非固定張數陣法：恰含其指定的兩種相剋屬性，每種至少一張牌，且各自有效等級總和至少為七。實際上五張手牌上限允許四或五張實體牌。依官方規則 2-5.5c，無論實際使用多少張牌，天劫絕不符合要求固定陣法張數的規則。

裂地崩山（`裂地崩山`）的來源用語 `環行牌`，指牌面屬性符合陣法轉移後環境的牌。因此新環境決定每位玩家可捨棄的牌。

施展玩家可選五種環境中的任意一種，包括目前環境。選擇相同屬性仍發出環境轉移，且仍須捨棄符合環境的牌。

官方規則 5-2.4i 使攻擊傷害與附帶效果同時發生。因此裂地崩山記錄宣告環境，從下家開始依序收集玩家選擇，答案之間不套用陣法結果。最後一個答案後，以一個原子 `AttackResolved` 轉移環境、套用所有棄牌或手牌展示、結算 60 點攻擊，再判定遊戲結果。選擇請求與回答事件仍是可重播的中間事實，不代表提前套用陣法效果。

鏽鐵枯林（`鏽鐵枯林`）對共用牌堆處理一次：展示牌堆頂八張、捨棄三級以上牌，再將其餘牌洗回共用牌堆。啟用個人牌組時分別處理各玩家牌堆。兩條路徑均重新排列牌堆中已有的牌，因此絕不回復順風次數。

團隊遊戲中，神算狀態使天雷劫火（`天雷劫火`）的 15 點全域扣除生命對狀態擁有者隊伍無效。隊伍生命值是玩家的生命值資源；若只將免疫套用於個人玩家，神算這部分在團隊遊戲中將無效果。

神算減少 20 點傷害只適用擁有者親自承受天劫攻擊傷害。若改由擁有者防護罩承受，防護罩承受未減少數值，因為玩家沒有承受該傷害。狀態仍使該天劫附帶效果對擁有者無效，並在天劫後移除。

神算陣法授予狀態後，由神算狀態自身擁有其保護；套用保護不會重複或續接授予陣法的效果。因此踏雪狀態不繞過神算的 20 點減傷，但踏雪仍依正常規則處理直接作用於攻擊的陣法效果。

神算不阻止鏽鐵枯林處理共用牌堆，因為該牌堆不屬於受保護玩家。啟用個人牌組時，只略過狀態擁有者的牌堆，正常處理其他玩家牌堆。

面對裂地崩山時，神算不阻止共享環境轉移。擁有者不捨棄環行牌也不展示手牌；所有其他玩家正常結算該部分效果。

泥石轟流（`泥石轟流`）僅在附帶效果實際從任意防護罩扣除至少一點時，攻擊點數由 60 增為 80。之後由攻擊本身造成的防護罩損失不能觸發增加。受神算保護的防護罩對此檢查沒有有效扣除。

烈風暴雨狀態（`烈風暴雨狀態`）依陣法施展玩家使生命回復無效，與江湖中毒先例一致。沒有烈風暴雨狀態的玩家施展陣法仍可回復其隊伍生命，即使隊友有該狀態。非陣法回復仍有效。

官方規則 6-1 分別管理每次烈風暴雨狀態持續時間。施展玩家目前回合結束算其第一個受影響回合；其他玩家各計算接下來兩次回合結束。重複施加作為獨立兩回合效果重疊，不合併或延長單一計數器。神算只防止其回應的天劫新施加狀態，不移除較早烈風暴雨狀態。

沒有環行牌時，裂地崩山使玩家**展示**手牌，不允許其他玩家**檢視**該手牌，因此不觸發惡精靈戲鬧。天雷劫火的 `both Teams` 扣除生命是陣法效果：所屬隊伍實際失去生命的每位存活死精靈擁有者均觸發同命。受神算保護的隊伍沒有此生命扣除，也不觸發同命。

可執行行動詳情是選用、依情境而定的玩家決策契約，不是陣法目錄 `rule_text`，也不是所提供行動的獨立重述。對迴響曲調，它補充可見選項的相關迴響政策：五種選用代價曲調說明允許的迴響代價屬性及有條件的下次回合開始重複；變徵‧淨火說明自動無代價重複；變宮‧植土說明其獨立的下次回合開始曲調主效果選擇。以「主效果重複一次」等正向文案表達玩家可見邊界，不必描述引擎是否建立另一個陣法施展。[ADR-0023](adr/0023-separate-action-detail-from-catalog-rule-text.md) 擁有具型別規則事實邊界；定義情境完整性的 [ADR-0028](adr/0028-treat-action-detail-as-contextual-supplement.md) 修訂該決策。

`SpiritLevelInterpreted.skill` 是選用正規語意欄位，記錄建立牌解讀的火精靈技能；公開視圖不會將缺席值猜測為螢光或絢爛。

### 41. Independent Initial Pouch Selection

初始錦囊選擇仍是規則引擎擁有、公開且可重新連線的遊戲準備階段，但不再指定單一預期玩家。每位尚未放置初始錦囊的玩家均可立即提交 `ChooseInitialPouch`。每個已接受命令依伺服器收到順序獨立提交正規事件；不將整組表示為一個待選擇事項或一個開啟中的多玩家結算交易。

從行動順序扣除已放置初始錦囊的擁有者，推導尚未選擇玩家；不在正規遊戲狀態持久化第二份 `remaining_players` 集合。驗證操作者位於行動順序、尚未選擇，且所選實體牌位於自己目前個人牌組。以新命令 ID 第二次提交時回傳特定 `InitialPouchAlreadyChosen` 驗證失敗，不發出事件；相同命令 ID 的重試仍受線上命令交易收據管轄。

每個已接受選擇依序發出 `PouchPlaced` 及 `InitialPouchChosen { player, card }`；從後者移除 `next_player`。最後一位尚未選擇者還會在第一個 `RandomnessRequested` 前發出 `InitialPouchSelectionCompleted`；只有該完成事件使遊戲準備進入 `PendingDeckShuffle`。剩餘個人牌組洗牌及初始發牌仍與原先一樣依行動順序執行。此政策適用兩種支援人數：雙人及四人。

選擇命令批次可交換：正規儲存仍序列化提交並驗證 `expectedSequence`，但因每個選擇改變不同玩家的牌堆及錦囊，最終規則引擎投影不受已接受選擇順序影響。無須正規回滾或順序正規化層。事件日誌刻意保留到達順序以供稽核及重播；每種合法順序重播均須投影相同最終狀態。

公開狀態以精確 camelCase 契約 `initialPouchSelection: { remainingPlayers: PlayerId[] } | null` 公開此階段。已完成玩家由行動順序扣除 `remainingPlayers` 推導；公開進度物件絕不加入牌身分。觀看者專屬互動僅在該觀看者尚未選擇時允許選擇。錦囊擁有者在提交及重新連線後可檢視所選牌，其他觀看者均看到牌背。Web 互動保留點牌及隨機快捷鍵立即提交，不需確認。

線上進行中遊戲回應攜帶由 `gameInstanceId` 與 `recordSequence` 組成的進行中遊戲版本。相同遊戲實例下，瀏覽器忽略序號低於已套用最高回應的回應。這是 HTTP 及 WebSocket 快照的傳送順序防護，不是正規並行機制；本變更不新增全域房間 revision。

### 42. Deep Secret Strategy Decisions

在 Rust 錦囊規則模組內以子模組深化秘計結算，負責秘計選項建構、完整決策驗證、立即秘計效果事件及秘計專屬續接。外層錦囊模組保留來源取得、錦囊翻開、放置及消耗、直接觸發與連環生命週期，以及連環檢索後洗牌。外部 `OfficialRules` 介面不變，不建立公開外掛或逐秘計擴充接點。

以單一封閉代數命令／參數物件取代目前寬泛選用欄位袋。`SecretStrategyDecision` 保留來源牌及下列恰好五類之一、符合答案結構的選擇：

- 無輸入秘計：金蟬、偷梁、混水、觀火、還魂及暗渡；
- 離山指定一位目標玩家；
- 瞞天使用 `Gain(star)` 或 `Break(star)`；
- 走為使用 `Clear` 或 `TransferByDiscard(card)`；
- 開始牽羊，不含牌堆或棄牌堆選牌。

封閉變體必須使無關欄位及非法欄位組合無法表示。尤其以星辰操作取代 `star + breakStar`，以環境操作取代選用棄牌。使用窮舉具型別 Rust 分派，不建立十個淺層秘計物件。直接觸發錦囊攜帶完整決策；連環答案使用獨立封閉外層變體：`PlaceOnly` 或包含相同完整決策的 `PlaceAndTrigger`。兩條路徑共享秘計語意，但不合併外層生命週期。

秘計選項遵循相同答案結構類別。每個選項只包含與其答案相關的候選：無輸入秘計沒有空候選集合；離山公開目標玩家；瞞天公開合法星辰操作；走為公開合法環境操作；牽羊只公開可開始延後交換的事實。移除通用 `requiredCardCount` 及無關空選項欄位。這些私密互動投影可跨越 Rust 至 TypeScript 邊界，但決策不是正規遊戲狀態，也不加入公開狀態或公開事件動態。

提交時依目前正規狀態重新驗證完整決策；沒有不透明 Option ID，也不信任先前選項快照。發出任何事件前驗證來源牌、依牌面值判定的所選秘計資格、目前操作者及目標權限，以及每個具型別操作。過期、偽造、不符或其他非法決策均為驗證失敗，正規事件為零，且不翻開、消耗、產生代價或移動。內部建立、已成功驗證但模組無法結算的決策，才屬規則實作錯誤。

瞞天 `Break(star)` 要求提交時該星辰存在；不存在或過期目標為驗證失敗，不是合法無效果結算。這不同於授予的暫時星辰效果，其陣法日後嘗試破除己方未擁有的同名星辰；後者仍是合法無效果結果。走為 `Clear` 在沒有目前環境時合法，並結算為無變化。`TransferByDiscard(card)` 要求目前手牌實體牌、讀取牌面屬性、允許公開外來牌、將牌送回牌來源棄牌堆；沒有目前環境或目前環境同屬性時仍合法。

牽羊初始決策只開始續接。必要的預先洗棄牌後，保留後續具型別牽羊待選擇事項：恰選目前牌堆兩張，將這些有序移動投影至棄牌堆，再從投影後棄牌堆恰選兩張。既有依序驗證允許任一剛捨棄牌被放回。交換及可信任牌堆洗牌完成前，秘計來源牌仍置於一旁且不可選。

所有可立即判定的驗證均先於正規輸出。直接有效結算依序為 `PouchRevealed`、秘計效果、`PouchConsumed`。連環依序為 `ChoiceMade`、`PouchPlaced`、`PouchRevealed`、秘計效果、`PouchConsumed`，必要時再執行正常檢索後洗牌。延後牽羊先翻開，直到具型別續接及洗牌完成才消耗；其洗牌仍取代連環原本額外的檢索後洗牌。

TypeScript 只是瀏覽器本機草稿至封閉命令 DTO 的介面卡，不得重作秘計資格或規則引擎驗證。Rust 至 TypeScript 帶標籤變體及所有新增多詞欄位均使用精確 camelCase 序列化契約，並有明確測試。

刪改既有錦囊測試時，依 ADR-0032 確認其獨有行為仍有適當測試覆蓋。保留互動矩陣及代表性線上房間與 Playwright 接點，同時增加原子驗證、正規順序、重播及序列化重點覆蓋。

### 43. Battle Record Turn-Relative Narration

將依觀看者過濾的公開決策動態投影為確定性的戰局紀錄，供即時房間及重播使用。`TurnStarted` 事實立即建立帶標籤的回合組，即使該組沒有項目；它是邊界而非獨立戰局紀錄項目。準備階段仍是獨立回合前組，可保留既有空組顯示政策。房間及重播視圖顯示空回合組，不加佔位項目；新組屬於供自動捲動的新可見內容。

回合組內只有具型別語意主詞為回合玩家時才省略主詞，不得從已呈現名稱全域刪除推導。敘述器改在各具型別玩家角色邊界選擇句子，使其他決策者，以及作為目標、擁有者、來源、隊伍參照或下一決策者所需的玩家均明確保留，並使用正常相對觀看者標籤（`你`、顯示名稱、`我方`／`對方`或觀戰者標籤）。準備階段沒有回合玩家，因此不省略玩家主詞。

每個已接受玩家決策仍是一個項目；其他玩家後續決策另起後續項目，其自動結果、蓋牌翻開及待隨機處理則保留於既有決策邊界。項目標題以公開具型別事實命名完整決策，例如陣法、精靈技能、職業能力或秘計。摘要只補充公開代價、選擇、原因及結果，並須使用官方屬性牌標籤格式（`金`／`木`／`水`／`火`／`土`、空格、等級）。不重述標題已命名的決策：陣法標題後可接 `使用火 2、水 1。`，具名精靈技能標題後可接靈力變化及其他公開結果。

戰局紀錄不是持久化呈現文字。舊重播以目前確定性敘述規則投影其依觀看者過濾的正規輸入，因此相同過濾輸入及視角一律產生相同文案。不為此投影新增 NLG、LLM 或 Fluent 依賴。

擴充既有戰局紀錄及開始 API 測試、捲動測試及戰場版面情境驗證此行為。涵蓋第一個項目前回合開始，以及同一玩家分別作為主詞、目標及擁有者。不需要專用 TurnStarted 整合測試或新 E2E 規格。

### 44. HP Change Planning

每次外層規則結算建立一份經驗證隊伍生命值帳本。帳本要求每隊各有目前值及初始值；無效範圍或算術溢位視為引擎不變條件錯誤，並記錄所有生命值變化嘗試，包括防止、界限限制及無變化。同一隊伍的重複變化依正規呼叫順序繼承前一筆規劃後新生命值。

一次已結算陣法效果取得有範圍的帳本工作階段。完成時只依正規 `state.hp` 順序回報實際失去生命的隊伍；同命之後由外層帳本規劃，不能遞迴進入該工作階段。迴響重複不是陣法效果；江湖狀態則擁有自身持續陣法效果時點。待處理續接從當時正規狀態建立新帳本，不序列化作用中的規劃器。

### 45. Totem Formation Rule Interpretations

來源：提供的五行戰鬥牌5.17規則書.pdf，印刷頁 38–39。本節記錄已接受的訪談判定；未解決互動在另行確認前不屬本節範圍。

一致使用青角圖騰。東靈陣‧青角結果中寫作青爪圖騰是筆誤，不是另一種圖騰。

五種授予圖騰的陣法各要求恰好三張其屬性牌，且至少一張解讀後等級恰為五。例如木1、木2、木5 符合東靈陣‧青角；木1、木2、木6 不符合。既有合法牌解讀只在其授予範圍內適用；此判定不將僅基礎規則的解讀能力擴大至主題陣法。

圖騰第一效果只在其玩家承受傷害時防止環境傷害加倍；防護罩承受時不適用。木行環境中，持有青角圖騰的玩家若有防護罩，20 點木行攻擊仍對防護罩造成 40 點；沒有防護罩時圖騰移除環境加倍。其他適用修正仍各自獨立。

只有虛空斷脈術成功破除既有環境才破除所有圖騰。沒有環境時施展、以走為破除環境，以及一般環境轉移均保留圖騰。

南靈陣‧朱羽先經主動術式有效性檢查。若無效，完整效果均不執行，包括轉為攻擊；若有效，轉為所選五行攻擊並經適用攻擊傷害檢查。因此封印可使完整陣法無效，防禦則可在轉換後防止其傷害。此處遵循既有幻化先作主動術式檢查、再轉換的處理。

圖騰第三效果在適用的環境傷害轉回復被防止時觸發並消耗該圖騰。防護罩承受攻擊、聖獸忽略圖騰，或傷害已因防禦等原因無效時不觸發。獨立五行相生也將攻擊轉為回復時，仍因防止環境轉換而消耗圖騰；相生造成的回復仍然存在。消耗為強制，不是玩家選項。

南靈陣‧朱羽使用原環境及原圖騰完成攻擊結算，再轉為火行環境並授予朱羽圖騰。例如火行環境下選擇木行攻擊，施展者取得朱羽圖騰前可能消耗既有青角圖騰。僅防止傷害不會阻止後續環境轉移或取得圖騰；初始主動術式無效則阻止全部效果。

圖騰第二效果使實際施展陣法身分不受環境無效影響，而非保護幻化複製的效果身分。黃鱗圖騰保護幻化；青角圖騰不會僅因幻化複製氣壁而保護幻化。封印等其他獨立無效依據仍然存在。

圖騰法陣規則模組依賴全部三個進階規則模組，但不要求個人牌組。尋龍在共用牌堆遊戲檢索共用牌堆，在個人牌組遊戲檢索施展玩家自己的牌堆。

反震先將攻擊分攤，再分別判定各承受者圖騰第一效果對環境加倍的例外。火行環境中，防守者持朱羽圖騰時，經反震的 20 點火行攻擊對無防護罩防守者造成 10 點；若攻擊方沒有圖騰，則對攻擊方直接造成 20 點。防守者例外不保護攻擊方；既有反彈份額直接扣除生命規則不變。

圖騰第三效果對完整攻擊判定一次，不對每個反震份額各判一次。青角圖騰防止木行攻擊在火行環境中轉為回復時，消耗圖騰一次並使兩個份額都維持傷害模式，除非獨立五行相生仍使攻擊回復生命。結算某一份額時消耗圖騰，絕不使同次攻擊的另一份額恢復環境回復。

陣法有效性在施展時決定，包括當時適用的傳入反制效果、環境及任何圖騰豁免。蓋牌的判定時點是覆蓋行動，不是日後翻開。因此圖騰第二效果參與施展時判定；之後環境或圖騰變更不追溯使已施展陣法無效或恢復有效。例如覆蓋有效防禦後，下家以走為轉為金行環境不會使該防禦無效。覆蓋時因金行環境無效的防禦，也不會僅因環境後來改變而恢復。翻開時對傳入行動的適用性及之後明確中和，仍與此判定分開。

訪談發現的實作缺口：`covered_passive::trigger` 目前針對翻開時即時狀態呼叫 `environment_makes_formation_ineffective`，而 `PassiveCovered` 只記錄 `sealed`，沒有施展時環境依據。須修正此行為，並保留依觀看者過濾的原因，不提前暴露隱藏陣法。回歸檢查涵蓋覆蓋與翻開間環境變更的兩個方向。既有金行環境／防禦測試使環境保持不變，無法區分時點。此缺口是已確認修正，不是未決規則問題。

尋龍所選牌先公開展示，剩餘牌堆洗牌後再反面放至牌堆頂。公開展示保留在戰局紀錄；但抽入手牌後遵循一般隱藏手牌規則，展示不建立永久公開可見性。既有公開外來牌可見性仍各自獨立適用。

中靈陣‧黃鱗檢視非空下家手牌後必須恰選一張，不得拒絕。手牌為空時略過選牌及手牌間移動，但仍轉為土行環境並授予黃鱗圖騰。完整術式無效時阻止所有效果。
