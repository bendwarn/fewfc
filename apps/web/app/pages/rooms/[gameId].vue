<template>
  <main v-if="!routeReady" class="route-state" aria-live="polite">
    <div class="route-state-content"><span class="route-spinner" aria-hidden="true" /><h1>正在載入</h1><p>正在恢復房間狀態。</p></div>
  </main>
  <main v-else-if="roomRouteError" class="route-state">
    <div class="route-state-content"><h1>{{ roomRouteError }}</h1><button class="primary-button" type="button" @click="returnToLobby">返回房間大廳</button></div>
  </main>
  <main v-else class="game-page">
    <div class="battle-layout">
        <section
          ref="battlefield"
          class="battlefield"
          :class="{
            'four-player': playerSeats.length === 4,
            'discard-open': discardOpen,
          }"
          aria-label="五行戰鬥牌對戰桌"
        >
          <button
            class="back-button battlefield-back"
            type="button"
            aria-label="返回房間列表"
            title="返回房間列表"
            @click="leaveGame"
          >
            ←
          </button>

          <div
            v-for="seat in playerSeats"
            :key="seat.player"
            class="player-seat"
            :class="[
              `seat-${seat.position}`,
              { acting: !gameFinished && state.currentPlayer === seat.player },
            ]"
          >
            <div class="player-identity">
              <span
                class="connection-dot"
                :class="{ connected: playerConnected(seat.player) }"
                :title="playerConnected(seat.player) ? '已連線' : '已斷線'"
              />
              <span class="avatar">{{ playerInitialFor(seat.player) }}</span>
              <div>
                <strong>{{ playerLabel(seat.player) }}</strong>
                <small>{{ teamHp(teamForPlayer(seat.player)) }} HP</small>
                <small v-if="teamStar(teamForPlayer(seat.player))">
                  星辰 · {{ starLabel(teamStar(teamForPlayer(seat.player))!) }}
                </small>
                <small
                  v-if="state.enabledRuleModules.includes('star') && starHistoryLabel(seat.player)"
                >
                  召星 · {{ starHistoryLabel(seat.player) }}
                </small>
                <small v-if="spiritFor(seat.player)" class="spirit-status">
                  精靈 · {{ spiritLabel(spiritFor(seat.player)!.spirit) }}
                  · 靈力 {{ spiritFor(seat.player)!.power }} / 6
                </small>
                <small v-if="state.pouches.some(pouch => pouch.owner === seat.player)">
                  錦囊 ·
                  {{ state.pouches.find(pouch => pouch.owner === seat.player)?.card?.label ?? '覆蓋牌' }}
                </small>
                <span
                  v-if="professionFor(seat.player)"
                  class="profession-badge"
                  tabindex="0"
                  :aria-label="professionSummaryLabel(seat.player)"
                >
                  職業 · {{ professionFor(seat.player)!.name }}
                  <span class="profession-summary" role="note">
                    <b>{{ professionFor(seat.player)!.name }}</b>
                    <small
                      v-for="ability in professionFor(seat.player)!.abilities"
                      :key="ability"
                    >
                      {{ ability }}
                    </small>
                  </span>
                </span>
                <small v-if="state.enabledRuleModules.includes('personal-deck')">
                  牌庫 {{ playerDeckCount(seat.player) }} · 棄牌 {{ playerDiscardCount(seat.player) }}
                </small>
                <small
                  v-for="card in exposedDeckCards(seat.player)"
                  :key="`exposed-deck-${seat.player}-${card.id}`"
                >
                  公開牌：{{ card.label }}
                </small>
              </div>
              <span
                v-if="seat.player === ownPlayer && game.connectionState.value === 'reconnecting'"
                class="reconnecting-label"
              >
                重新連線中
              </span>
              <span
                v-if="state.currentPlayer === seat.player && !roomWaiting && !gameFinished"
                class="turn-badge"
              >
                行動中 · {{ phaseLabel(state.phase) }}
              </span>
              <span
                v-for="counter in counterEffectsFor(seat.player)"
                :key="`${seat.player}-${counter.effectId}`"
                class="counter-badge"
              >
                反制 · {{ counter.effectName }}
              </span>
              <span
                v-if="shieldFor(seat.player) > 0"
                class="shield-badge"
              >
                防護罩 · {{ shieldFor(seat.player) }}
              </span>
              <span
                v-for="effect in persistentEffectsFor(seat.player)"
                :key="`${seat.player}-${effect.key}`"
                class="status-badge persistent-effect"
              >
                {{ effect.label }}
              </span>
              <span class="side-hand-count">{{ handCount(seat.player) }} 張</span>
            </div>

            <div class="hand fan seat-hand">
              <GameCard
                v-for="(card, cardIndex) in cardsFor(seat.player)"
                :key="card.id"
                :card="card.card"
                :hidden="card.hidden"
                :selectable="card.selectable"
                :selected="card.selected"
                :disabled="!roomConnected"
                :interpretations="state.cardInterpretations"
                :data-card-id="card.cardId"
                :shortcut="card.selectable
                  ? shortcutLabel(HAND_SHORTCUT_KEYS, cardIndex)
                  : undefined"
                @select="game.toggleCardSelection(seat.player, card.cardId)"
              />
            </div>
          </div>

          <div class="board-center">
            <div
              class="discard-piles"
              :class="{ personal: state.playerDiscards.length > 0 }"
              aria-label="棄牌堆"
            >
              <DiscardPileControl
                v-for="pile in visibleDiscardPiles"
                :key="pile.owner ?? 'shared'"
                :owner="pile.owner"
                :owner-label="pile.owner ? playerLabel(pile.owner) : ''"
                :count="pile.cards.length"
                :unavailable="discardPileUnavailable(pile.cards)"
                :open="discardOpen && activeDiscardOwner === pile.owner"
                :position="pile.position"
                @toggle="toggleDiscardComposition(pile.owner, $event)"
              />
            </div>
            <div class="formation-field">
              <div class="formation-field-heading">
                <span class="formation-field-label">陣法區</span>
                <span v-if="state.environment" class="environment-badge" aria-live="polite">
                  環境 · {{ environmentLabel(state.environment) }}
                </span>
              </div>
              <div class="previous-formation">
                <template v-if="state.previousTurnFormation">
                  <small>上一回合 · {{ playerLabel(state.previousTurnFormation.player) }}</small>
                  <strong>{{ state.previousTurnFormation.formationName ?? '蓋牌' }}</strong>
                  <div class="formation-cards">
                    <GameCard
                      v-for="card in previousFormationCards"
                      :key="card.id"
                      class="formation-card"
                      :card="card.card"
                      :hidden="card.hidden"
                      :interpretations="state.cardInterpretations"
                    />
                  </div>
                </template>
                <p v-else>上一回合未發動陣法</p>
              </div>

              <div
                v-if="!roomWaiting && !gameFinished && viewer === state.currentPlayer"
                class="turn-controls"
              >
                <p v-if="game.isLoading.value" class="action-processing">處理中</p>
                <p v-else-if="game.errorMessage.value" class="action-error">
                  {{ game.errorMessage.value }}
                </p>

                <section class="ability-panel" aria-labelledby="ability-panel-title">
                  <header>
                    <h3 id="ability-panel-title">能力</h3>
                    <small>不結束行動階段</small>
                  </header>
                  <div class="action-candidates">
                    <button
                      v-for="(pouchAction, abilityIndex) in pouchStrategyActions"
                      :key="`pouch-${pouchAction.label}`"
                      type="button"
                      :title="pouchAction.detail || undefined"
                      :aria-label="labelWithDetail(pouchAction.label, pouchAction.detail)"
                      :aria-keyshortcuts="shortcutLabel(ABILITY_SHORTCUT_KEYS, abilityIndex)"
                      data-keyboard-shortcut="ability"
                      :disabled="!roomConnected || game.isLoading.value"
                      @mouseenter="showTextActionDetail(pouchAction.detail)"
                      @mouseleave="hideActionDetail"
                      @focus="showTextActionDetail(pouchAction.detail)"
                      @blur="hideActionDetail"
                      @click="startPouchAction(pouchAction)"
                    >
                      {{ pouchAction.label }}
                    </button>
                    <button
                      v-for="(ability, abilityIndex) in directPlayableAbilities"
                      :key="playableAbilityKey(ability)"
                      type="button"
                      :title="playableActionDetail(ability) || undefined"
                      :aria-label="playableActionAccessibleLabel(ability)"
                      :aria-keyshortcuts="shortcutLabel(
                        ABILITY_SHORTCUT_KEYS,
                        directAbilityShortcutOffset + abilityIndex,
                      )"
                      data-keyboard-shortcut="ability"
                      @mouseenter="showActionDetail(ability)"
                      @mouseleave="hideActionDetail"
                      @focus="showActionDetail(ability)"
                      @blur="hideActionDetail"
                      @click="startDirectAbility(ability)"
                    >
                      {{ ability.name }}
                    </button>
                    <div
                      v-if="darkSpiritPicker"
                      class="spirit-level-picker"
                      role="group"
                      aria-label="暗靈：選擇指定等級"
                      @click.stop
                    >
                      <button
                        ref="darkSpiritMenuTrigger"
                        class="spirit-level-trigger"
                        type="button"
                        :title="playableActionDetail(darkSpiritPicker.representative) || undefined"
                        :aria-label="playableActionAccessibleLabel(darkSpiritPicker.representative)"
                        :aria-keyshortcuts="shortcutLabel(
                          ABILITY_SHORTCUT_KEYS,
                          darkSpiritShortcutIndex,
                        )"
                        data-keyboard-shortcut="ability"
                        aria-haspopup="menu"
                        :aria-expanded="darkSpiritMenuOpen"
                        @mouseenter="showActionDetail(darkSpiritPicker.representative)"
                        @mouseleave="hideActionDetail"
                        @focus="showActionDetail(darkSpiritPicker.representative)"
                        @blur="hideActionDetail"
                        @click="darkSpiritMenuOpen = !darkSpiritMenuOpen"
                      >
                        暗靈
                      </button>
                      <div v-if="darkSpiritMenuOpen" class="spirit-level-options" role="menu">
                        <button
                          v-for="ability in darkSpiritPicker.options"
                          :key="playableAbilityKey(ability)"
                          type="button"
                          role="menuitem"
                          :title="playableActionDetail(ability) || undefined"
                          :aria-label="labelWithDetail(`暗靈：指定為 ${ability.declaredLevel} 級`, playableActionDetail(ability))"
                          @mouseenter="showActionDetail(ability)"
                          @mouseleave="hideActionDetail"
                          @focus="showActionDetail(ability)"
                          @blur="hideActionDetail"
                          @click="startDarkSpiritAction(ability)"
                        >
                          {{ ability.declaredLevel }} 級
                        </button>
                      </div>
                    </div>
                    <div
                      v-if="splendorPicker"
                      class="spirit-level-picker"
                      role="group"
                      aria-label="絢爛：選擇指定等級"
                      @click.stop
                    >
                      <button
                        ref="splendorMenuTrigger"
                        class="spirit-level-trigger"
                        type="button"
                        :title="playableActionDetail(splendorPicker.representative) || undefined"
                        :aria-label="playableActionAccessibleLabel(splendorPicker.representative)"
                        :aria-keyshortcuts="shortcutLabel(
                          ABILITY_SHORTCUT_KEYS,
                          splendorShortcutIndex,
                        )"
                        data-keyboard-shortcut="ability"
                        aria-haspopup="menu"
                        :aria-expanded="splendorMenuOpen"
                        @mouseenter="showActionDetail(splendorPicker.representative)"
                        @mouseleave="hideActionDetail"
                        @focus="showActionDetail(splendorPicker.representative)"
                        @blur="hideActionDetail"
                        @click="splendorMenuOpen = !splendorMenuOpen"
                      >
                        絢爛
                      </button>
                      <div v-if="splendorMenuOpen" class="spirit-level-options" role="menu">
                        <button
                          v-for="ability in splendorPicker.options"
                          :key="playableAbilityKey(ability)"
                          type="button"
                          role="menuitem"
                          :title="playableActionDetail(ability) || undefined"
                          :aria-label="labelWithDetail(`絢爛：指定為 ${ability.declaredLevel} 級`, playableActionDetail(ability))"
                          @mouseenter="showActionDetail(ability)"
                          @mouseleave="hideActionDetail"
                          @focus="showActionDetail(ability)"
                          @blur="hideActionDetail"
                          @click="startSplendorAction(ability)"
                        >
                          {{ ability.declaredLevel }} 級
                        </button>
                      </div>
                    </div>
                    <button
                      v-if="game.playableDiscardRetrieval.value"
                      class="retrieve-action"
                      type="button"
                      :title="discardRetrievalDetail || undefined"
                      :aria-label="labelWithDetail('棄牌回收', discardRetrievalDetail)"
                      :aria-keyshortcuts="shortcutLabel(
                        ABILITY_SHORTCUT_KEYS,
                        discardRetrievalShortcutIndex,
                      )"
                      data-keyboard-shortcut="ability"
                      :disabled="!roomConnected"
                      @mouseenter="showTextActionDetail(discardRetrievalDetail)"
                      @mouseleave="hideActionDetail"
                      @focus="showTextActionDetail(discardRetrievalDetail)"
                      @blur="hideActionDetail"
                      @click="game.performPlayableAction(game.playableDiscardRetrieval.value)"
                    >
                      棄牌回收
                    </button>
                    <p v-if="!game.playableAbilities.value.length && !game.playableDiscardRetrieval.value">
                      目前沒有可用能力
                    </p>
                  </div>
                </section>

                <section class="action-panel" aria-labelledby="action-panel-title">
                  <header>
                    <h3 id="action-panel-title">行動</h3>
                    <small>使用後結束行動階段</small>
                  </header>
                  <div v-if="!game.isLoading.value" class="action-candidates">
                    <button
                      v-for="(action, actionIndex) in game.playableMainActions.value"
                      :key="`${action.type}:${action.id}:${action.cards.join('-')}:${action.type === 'performFormation' ? `${action.starSubstitution?.card ?? 'printed'}:${action.matchOption?.role ?? 'default'}:${action.matchOption?.card ?? ''}` : 'profession'}`"
                      type="button"
                      :title="playableActionDetail(action) || undefined"
                      :aria-label="playableActionAccessibleLabel(action)"
                      :aria-keyshortcuts="shortcutLabel(ACTION_SHORTCUT_KEYS, actionIndex)"
                      data-keyboard-shortcut="action"
                      @mouseenter="showActionDetail(action)"
                      @mouseleave="hideActionDetail"
                      @focus="showActionDetail(action)"
                      @blur="hideActionDetail"
                      @pointerdown="startActionDetail(action)"
                      @pointerup="cancelActionDetail"
                      @pointercancel="cancelActionDetail"
                      @click="startPlayableAction(action)"
                    >
                      {{ playableActionName(action) }}
                    </button>
                    <button
                      v-if="showSkip"
                      class="skip-action"
                      type="button"
                      :disabled="!roomConnected"
                      :aria-keyshortcuts="shortcutLabel(
                        ACTION_SHORTCUT_KEYS,
                        game.playableMainActions.value.length,
                      )"
                      data-keyboard-shortcut="action"
                      @click="game.playablePass.value && game.performPlayableAction(game.playablePass.value)"
                    >
                      跳過
                    </button>
                  </div>
                  <p
                    v-if="!game.isLoading.value && !game.playableMainActions.value.length && !game.errorMessage.value"
                    class="action-prompt"
                  >
                    選擇手牌以尋找可用行動
                  </p>
                </section>

                <p v-if="actionDetail" class="action-detail">{{ actionDetail }}</p>
              </div>
            </div>
            <div
              v-if="discardOpen"
              class="discard-composition-layer"
            >
                <section
                  id="discard-composition"
                  class="card-composition discard-composition"
                  role="dialog"
                  aria-labelledby="discard-composition-title"
                >
                  <h2 id="discard-composition-title">棄牌內容</h2>
                  <table>
                    <caption class="sr-only">依五行與等級統計棄牌張數。列為五行，欄為等級。</caption>
                    <thead>
                      <tr>
                        <th scope="col"><span class="sr-only">五行</span></th>
                        <th
                          v-for="level in CARD_LEVELS"
                          :key="`discard-heading-${level}`"
                          scope="col"
                        >
                          {{ level }} 級
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="row in discardComposition" :key="`discard-element-${row.element}`">
                        <th scope="row">{{ row.element }}</th>
                        <td
                          v-for="cell in row.cells"
                          :key="`${cell.element}-${cell.level}`"
                          :class="{ empty: cell.count === 0 }"
                        >
                          <span class="sr-only">{{ cell.element }} {{ cell.level }} 級，共 {{ cell.count }} 張</span>
                          <strong aria-hidden="true">{{ cell.count }}</strong>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </section>
            </div>
          </div>

          <div
            v-if="virtualFormationCardDraft"
            class="choice-overlay"
            role="dialog"
            aria-modal="true"
            aria-labelledby="virtual-formation-card-title"
          >
            <div ref="virtualFormationCardDialog" class="virtual-formation-card-dialog">
              <h2 id="virtual-formation-card-title">
                {{ virtualFormationCardDraft.name }}：選擇虛擬牌
              </h2>
              <table class="virtual-formation-card-matrix">
                <caption class="sr-only">列為五行，欄為等級；選擇後立即發動能力。</caption>
                <thead>
                  <tr>
                    <th scope="col"><span class="sr-only">五行</span></th>
                    <th
                      v-for="level in virtualFormationCardDraft.inputRequirement.levels"
                      :key="`virtual-level-${level}`"
                      scope="col"
                    >
                      {{ level }} 級
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="element in virtualFormationCardDraft.inputRequirement.elements"
                    :key="`virtual-element-${element}`"
                  >
                    <th scope="row">{{ environmentLabel(element) }}</th>
                    <td
                      v-for="level in virtualFormationCardDraft.inputRequirement.levels"
                      :key="`${element}-${level}`"
                    >
                      <button
                        class="virtual-formation-card-option"
                        type="button"
                        :disabled="game.isLoading.value || !roomConnected"
                        :aria-label="`${virtualFormationCardDraft.name}：${environmentLabel(element)} ${level} 級`"
                        @click="chooseVirtualFormationCard(element, level)"
                      >
                        {{ level }}
                      </button>
                    </td>
                  </tr>
                </tbody>
              </table>
              <button
                class="virtual-formation-card-cancel"
                type="button"
                :disabled="game.isLoading.value"
                @click="closeVirtualFormationCardDraft()"
              >
                取消
              </button>
            </div>
          </div>

          <div
            v-if="state.status === 'Preparing' && game.interaction.value.canChooseInitialPouch"
            class="choice-overlay"
            role="dialog"
            aria-label="選擇初始錦囊"
          >
            <div>
              <h2>選擇初始錦囊</h2>
              <p>先從個人牌組選一張牌；所有玩家完成後才洗牌發牌。</p>
              <CardChoiceMatrix
                :cards="ownDeckCards"
                :disabled="game.isLoading.value || !roomConnected"
                label="初始錦囊牌組矩陣"
                caption="依五行與等級選擇初始錦囊"
                action-label="選擇作為初始錦囊"
                @choose="chooseInitialPouchCard"
              />
            </div>
          </div>

          <div
            v-else-if="state.status === 'Preparing'"
            class="choice-waiting-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>等待選擇錦囊</h2>
              <template v-if="state.initialPouchSelection">
                <p>
                  等待 {{ state.initialPouchSelection.remainingPlayers.map(playerLabel).join('、') }} 完成選擇。
                </p>
                <p>
                  已完成：{{ state.turnOrder.filter(player => !state.initialPouchSelection!.remainingPlayers.includes(player)).map(playerLabel).join('、') || '尚無' }}
                </p>
              </template>
              <p v-else>所有玩家已完成選擇，伺服器正在洗牌與發牌。</p>
            </div>
          </div>

          <div
            v-if="secretStrategyDraft"
            class="choice-overlay"
            role="dialog"
            :aria-label="`秘計‧${strategyLabel(secretStrategyDraft.strategy)}：選擇輸入`"
          >
            <div>
              <h2>秘計‧{{ strategyLabel(secretStrategyDraft.strategy) }}</h2>
              <p class="action-detail">{{ presentSecretStrategyOption(secretStrategyDraft) }}</p>

              <div
                v-if="secretStrategyDraft.input === 'targetPlayer'"
                class="choice-options"
                aria-label="離山目標"
              >
                <button
                  v-for="player in secretStrategyDraft.targetPlayers"
                  :key="`secret-strategy-target-${player}`"
                  type="button"
                  :class="{ selected: secretStrategyTargetSelection === player }"
                  :aria-pressed="secretStrategyTargetSelection === player"
                  :disabled="game.isLoading.value || !roomConnected"
                  @click="secretStrategyTargetSelection = player"
                >
                  {{ playerLabel(player) }}
                </button>
              </div>

              <template v-if="secretStrategyDraft.input === 'star'">
                <h3>瞞天：取得星辰效果或破除星辰</h3>
                <div class="choice-options" aria-label="瞞天選擇">
                  <button
                    v-for="star in secretStrategyDraft.stars"
                    :key="`secret-strategy-star-${star}`"
                    type="button"
                    :class="{ selected: secretStrategyStarSelection === star && !secretStrategyBreakStar }"
                    :aria-pressed="secretStrategyStarSelection === star && !secretStrategyBreakStar"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="secretStrategyStarSelection = star; secretStrategyBreakStar = false"
                  >
                    取得 {{ starLabel(star) }}
                  </button>
                  <button
                    v-for="star in secretStrategyDraft.breakStars"
                    :key="`secret-strategy-break-star-${star}`"
                    type="button"
                    :class="{ selected: secretStrategyStarSelection === star && secretStrategyBreakStar }"
                    :aria-pressed="secretStrategyStarSelection === star && secretStrategyBreakStar"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="secretStrategyStarSelection = star; secretStrategyBreakStar = true"
                  >
                    破除 {{ starLabel(star) }}
                  </button>
                </div>
              </template>

              <template v-if="secretStrategyDraft.input === 'retreat'">
                <h3>走為：破除環境或捨棄手牌</h3>
                <div class="choice-options" aria-label="走為選擇">
                  <button
                    type="button"
                    :class="{ selected: secretStrategyRetreatSelection === 'clearEnvironment' }"
                    :aria-pressed="secretStrategyRetreatSelection === 'clearEnvironment'"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="secretStrategyRetreatSelection = 'clearEnvironment'"
                  >
                    破除環境
                  </button>
                  <button
                    v-for="card in ownHandCards.filter(
                      candidate => secretStrategyDraft?.handCards.includes(candidate.id),
                    )"
                    :key="`secret-strategy-hand-${card.id}`"
                    type="button"
                    :class="{ selected: secretStrategyRetreatSelection === card.id }"
                    :aria-pressed="secretStrategyRetreatSelection === card.id"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="secretStrategyRetreatSelection = card.id"
                  >
                    捨棄 {{ card.label }}
                  </button>
                </div>
              </template>

              <div class="choice-options choice-actions">
                <button
                  class="choice-confirm"
                  type="button"
                  :disabled="!canSubmitSecretStrategyDraft || game.isLoading.value || !roomConnected"
                  @click="submitSecretStrategyDraft"
                >
                  確認
                </button>
                <button type="button" :disabled="game.isLoading.value" @click="resetSecretStrategyDraft">
                  取消
                </button>
              </div>
            </div>
          </div>

          <div
            v-if="pouchChoiceKind"
            class="choice-overlay"
            role="dialog"
            :aria-label="pouchChoiceKind === 'chain' ? '連環：選擇錦囊' : '牽羊：交換牌'"
          >
            <div>
              <h2>{{ pouchChoiceKind === 'chain' ? '連環：選擇牌組牌' : '牽羊：交換牌' }}</h2>
              <p v-if="pouchChoiceKind === 'chain'">
                先依五行與等級選一張作為錦囊；也可再選一張公開觸發秘計。
              </p>
              <p v-else>各選 {{ pouchSwapRequiredCount }} 張牌組牌與棄牌交換，之後洗牌。</p>

              <template v-if="pouchChoiceKind === 'chain'">
                <h3>選擇錦囊牌</h3>
                <p v-if="chainPouchCard" class="choice-selection-summary">
                  已選：{{ chainPouchCard.label }}
                  <button type="button" @click="clearChainPouchCard">重新選擇</button>
                </p>
                <CardChoiceMatrix
                  class="chain-composition"
                  :cards="chainPouchCards"
                  :selected-cards="pouchDeckSelection.slice(0, 1)"
                  label="連環錦囊牌組矩陣"
                  caption="依五行與等級選擇連環錦囊牌"
                  action-label="選擇作為連環錦囊"
                  @choose="chooseChainPouchCard"
                />

                <template v-if="chainPouchCard">
                  <h3>選擇觸發牌（可選）</h3>
                  <p v-if="chainTriggerCard" class="choice-selection-summary">
                    已選：{{ chainTriggerCard.label }}
                    <button type="button" @click="clearChainTriggerCard">不觸發秘計</button>
                  </p>
                  <CardChoiceMatrix
                    class="chain-composition"
                    :cards="chainTriggerCards"
                    :selected-cards="pouchDeckSelection.slice(1, 2)"
                    label="連環觸發牌組矩陣"
                    caption="依五行與等級選擇連環觸發牌"
                    action-label="選擇作為連環觸發牌"
                    @choose="chooseChainTriggerCard"
                  />
                </template>
              </template>

              <template v-else>
                <CardChoiceMatrix
                  :cards="pouchSwapSelectableDeckCards"
                  :selected-cards="pouchDeckSelection"
                  :maximum="pouchSwapRequiredCount"
                  mode="toggle"
                  label="牽羊牌組矩陣"
                  caption="依五行與等級選擇牽羊交換的牌組牌"
                  action-label="選擇牽羊牌組牌"
                  @choose="togglePouchCard('pouchDeck', $event, pouchSwapRequiredCount)"
                />
                <h3>選擇 {{ pouchSwapRequiredCount }} 張要放回牌組的牌</h3>
                <CardChoiceMatrix
                  :cards="pouchSwapReturnCards"
                  :selected-cards="pouchDiscardSelection"
                  :maximum="pouchSwapRequiredCount"
                  mode="toggle"
                  label="牽羊回收矩陣"
                  caption="依五行與等級選擇牽羊放回牌組的牌"
                  action-label="選擇牽羊回收牌"
                  @choose="togglePouchCard('pouchDiscard', $event, pouchSwapRequiredCount)"
                />
              </template>

              <template v-if="pouchChoiceKind === 'chain'">
                <h3>錦囊持有者</h3>
                <div class="choice-options" aria-label="選擇錦囊持有者">
                  <button
                    v-for="player in chainPouchOwners"
                    :key="`pouch-owner-${player}`"
                    type="button"
                    :class="{ selected: pouchOwnerSelection === player }"
                    :aria-pressed="pouchOwnerSelection === player"
                    @click="pouchOwnerSelection = player"
                  >
                    {{ playerLabel(player) }}
                  </button>
                </div>

                <template v-if="chainTriggerCard">
                  <h3>觸發秘計</h3>
                  <div class="choice-options" aria-label="選擇秘計">
                    <button
                      v-for="option in chainStrategyOptions"
                      :key="`chain-strategy-${option.strategy}`"
                      type="button"
                      :class="{ selected: chainStrategySelection === option.strategy }"
                      :aria-pressed="chainStrategySelection === option.strategy"
                      @click="chainStrategySelection = option.strategy"
                    >
                      {{ strategyLabel(option.strategy) }}
                    </button>
                  </div>

                  <p v-if="selectedChainStrategyAction" class="action-detail">
                    {{ presentSecretStrategyOption(selectedChainStrategyAction) }}
                  </p>

                  <div
                    v-if="selectedChainStrategyAction?.input === 'targetPlayer'"
                    class="choice-options"
                    aria-label="離山目標"
                  >
                    <button
                      v-for="player in selectedChainStrategyAction.targetPlayers"
                      :key="`strategy-target-${player}`"
                      type="button"
                      :class="{ selected: strategyTargetSelection === player }"
                      :aria-pressed="strategyTargetSelection === player"
                      @click="strategyTargetSelection = player"
                    >
                      {{ playerLabel(player) }}
                    </button>
                  </div>

                  <p v-if="selectedChainStrategyAction?.input === 'deckDiscardSwap'">
                    確認後會先依當前牌組狀態洗棄牌（若需要），再選擇牽羊交換的牌。
                  </p>

                  <template v-if="selectedChainStrategyAction?.input === 'star'">
                    <h3>瞞天：取得星辰效果或破除星辰</h3>
                    <div class="choice-options" aria-label="瞞天選擇">
                      <button
                        v-for="star in selectedChainStrategyAction.stars"
                        :key="`strategy-star-${star}`"
                        type="button"
                        :class="{ selected: strategyStarSelection === star && !strategyBreakStar }"
                        :aria-pressed="strategyStarSelection === star && !strategyBreakStar"
                        @click="strategyStarSelection = star; strategyBreakStar = false"
                      >
                        取得 {{ starLabel(star) }}
                      </button>
                      <button
                        v-for="star in selectedChainStrategyAction.breakStars"
                        :key="`strategy-break-star-${star}`"
                        type="button"
                        :class="{ selected: strategyStarSelection === star && strategyBreakStar }"
                        :aria-pressed="strategyStarSelection === star && strategyBreakStar"
                        @click="strategyStarSelection = star; strategyBreakStar = true"
                      >
                        破除 {{ starLabel(star) }}
                      </button>
                    </div>
                  </template>

                  <template v-if="selectedChainStrategyAction?.input === 'retreat'">
                    <h3>走為：破除環境或捨棄手牌</h3>
                    <div class="choice-options" aria-label="走為選擇">
                      <button
                        type="button"
                        :class="{ selected: strategyDiscardCard === null }"
                        :aria-pressed="strategyDiscardCard === null"
                        @click="strategyDiscardCard = null"
                      >
                        破除環境
                      </button>
                      <button
                        v-for="card in ownHandCards.filter(
                          candidate => selectedChainStrategyAction?.handCards.includes(candidate.id),
                        )"
                        :key="`strategy-hand-${card.id}`"
                        type="button"
                        :class="{ selected: strategyDiscardCard === card.id }"
                        :aria-pressed="strategyDiscardCard === card.id"
                        @click="strategyDiscardCard = card.id"
                      >
                        捨棄 {{ card.label }}
                      </button>
                    </div>
                  </template>
                </template>
              </template>

              <div class="choice-options choice-actions">
                <button
                  class="choice-confirm"
                  type="button"
                  :disabled="!canSubmitPouchChoice || game.isLoading.value || !roomConnected"
                  @click="submitPouchChoice"
                >
                  確認
                </button>
                <button v-if="!state.pendingChoice" type="button" @click="resetPouchChoice">取消</button>
              </div>
            </div>
          </div>

          <div
            v-if="state.pendingChoice?.visibility === 'visible'
              && !pouchChoiceKind
              && viewer === state.pendingChoice.player"
            class="choice-overlay"
          >
            <div>
              <h2>{{ pendingChoiceLabel(state.pendingChoice) }}</h2>
              <template v-if="state.pendingChoice.choice.type === 'card'">
                <CardChoiceMatrix
                  v-if="state.pendingChoice.reason.type === 'echoRingingMetalDeckCard'"
                  :cards="state.pendingChoice.choice.cards"
                  :selected-cards="game.selectedChoiceCards.value"
                  :maximum="state.pendingChoice.choice.maximum"
                  :disabled="game.isLoading.value || !roomConnected"
                  mode="toggle"
                  label="商調‧鳴金牌組矩陣"
                  caption="依五行與等級選擇商調‧鳴金檢索的牌組牌"
                  action-label="選擇商調‧鳴金牌組牌"
                  @choose="game.choosePendingCard"
                />
                <div v-else class="choice-cards">
                  <GameCard
                    v-for="card in state.pendingChoice.choice.cards"
                    :key="card.id"
                    class="choice-card"
                    :card="card"
                    selectable
                    :selected="!isImmediateCardChoice(state.pendingChoice.choice)
                      && game.selectedChoiceCards.value.includes(card.id)"
                    :interpretations="state.cardInterpretations"
                    :disabled="game.isLoading.value || !roomConnected
                      || (!isImmediateCardChoice(state.pendingChoice.choice)
                        && game.selectedChoiceCards.value.length >= state.pendingChoice.choice.maximum
                        && !game.selectedChoiceCards.value.includes(card.id))"
                    @select="game.choosePendingCard(card.id)"
                  />
                </div>
                <p
                  v-if="!isImmediateCardChoice(state.pendingChoice.choice)"
                  class="choice-count"
                >
                  {{ cardChoiceDraftCount(
                    state.pendingChoice.choice,
                    game.selectedChoiceCards.value.length,
                  ) }}
                </p>
                <button
                  v-if="!isImmediateCardChoice(state.pendingChoice.choice)"
                  class="choice-submit"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected || !game.canSubmitPendingChoice.value"
                  @click="game.submitPendingChoice()"
                >
                  確認選擇
                </button>
                <button
                  v-if="state.pendingChoice.reason.type === 'clearWind'"
                  class="choice-submit"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  @click="game.discardClearWindCard()"
                >
                  捨棄此牌
                </button>
              </template>
              <div
                v-if="state.pendingChoice.choice.type === 'player'"
                class="choice-options"
                aria-label="選擇玩家"
              >
                <button
                  v-for="player in state.pendingChoice.choice.players"
                  :key="`choice-player-${player}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇玩家 ${playerLabel(player)}`"
                  @click="game.choosePendingPlayer(player)"
                >
                  {{ playerLabel(player) }}
                </button>
              </div>
              <SplitEarthFormationChoice
                v-if="state.pendingChoice.choice.type === 'formation'
                  && usesSplitEarthFormationGroups(state.pendingChoice.choice)"
                :key="splitEarthChoiceKey(state.pendingChoice.choice, state.turnNumber, roomConnected)"
                :groups="state.pendingChoice.choice.formationGroups"
                :disabled="game.isLoading.value || !roomConnected"
                @select="game.choosePendingFormation"
              />
              <div
                v-else-if="state.pendingChoice.choice.type === 'formation'
                  && state.pendingChoice.choice.formations.length > 0"
                class="choice-options"
                aria-label="選擇陣法"
              >
                <button
                  v-for="formationId in state.pendingChoice.choice.formations"
                  :key="`choice-formation-${formationId}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇陣法 ${formationChoiceLabel(formationId)}`"
                  @click="game.choosePendingFormation(formationId)"
                >
                  {{ formationChoiceLabel(formationId) }}
                </button>
              </div>
              <div
                v-if="state.pendingChoice.choice.type === 'environment'"
                class="choice-options"
                aria-label="選擇環境"
              >
                <button
                  v-for="environment in state.pendingChoice.choice.environments"
                  :key="`choice-environment-${environment}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇環境 ${environmentLabel(environment)}`"
                  @click="game.choosePendingEnvironment(environment)"
                >
                  {{ environmentLabel(environment) }}
                </button>
              </div>
              <button
                v-if="'canDecline' in state.pendingChoice.choice && state.pendingChoice.choice.canDecline"
                class="choice-submit"
                type="button"
                :disabled="game.isLoading.value || !roomConnected"
                @click="game.declinePendingChoice()"
              >
                放棄迴響
              </button>
            </div>
          </div>

          <div
            v-if="state.pendingChoice && !pouchChoiceKind
              && (state.pendingChoice.visibility === 'hidden' || viewer !== state.pendingChoice.player)"
            class="choice-waiting-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>等待 {{ playerLabel(state.pendingChoice.player) }}</h2>
              <p>{{ pendingChoiceLabel(state.pendingChoice) }}</p>
            </div>
          </div>

          <div
            v-if="state.pendingRandomness"
            class="choice-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>伺服器正在洗牌</h2>
              <p>
                正在安全地{{ state.pendingRandomness.operation === 'discardShuffle' ? '洗棄牌並放回牌組' : '洗牌組' }}
                {{ state.pendingRandomness.cardCount }} 張牌。
              </p>
            </div>
          </div>

          <div v-if="showSetupReveal" class="setup-reveal">
            <div>
              <h2>{{ onlineMetadata?.capacity === 4 ? '隊伍與行動順序' : '行動順序' }}</h2>
              <div v-if="onlineMetadata?.capacity === 4" class="revealed-teams">
                <span v-for="team in activeTeams" :key="team">
                  <strong>{{ teamLabel(team) }}</strong>
                  {{ teamMembers(team).join('、') }}
                </span>
              </div>
              <ol>
                <li v-for="player in state.turnOrder" :key="player">{{ playerLabel(player) }}</li>
              </ol>
            </div>
          </div>

          <WaitingRoomLayout v-if="roomWaiting">
            <template #main>
                <h2>{{ activeRoomName }}</h2>
                <p>{{ waitingRoomSummary }}</p>
                <p v-if="game.lockedDeckName.value" class="muted">
                  本局使用：{{ game.lockedDeckName.value }}
                </p>
                <RuleModuleSettings
                  :catalog="rulesCatalog.catalog.value?.ruleModules ?? []"
                  :enabled-rule-modules="onlineMetadata?.enabledRuleModules ?? []"
                  :is-owner="isRoomOwner"
                  :disabled="game.isLoading.value"
                  @update:enabled-rule-modules="updateWaitingRuleModules"
                />
                <p v-if="game.errorMessage.value" class="form-error" role="alert">
                  {{ game.errorMessage.value }}
                </p>
            </template>

            <template #side>
                <div class="waiting-members">
                  <span
                    v-for="player in onlinePlayers"
                    :key="player"
                    :class="{ joined: Boolean(memberForPlayer(player)) }"
                  >
                    {{ playerLabel(player) }}
                    <small>{{ waitingMemberStatus(memberForPlayer(player)) }}</small>
                    <button
                      v-if="isRoomOwner && memberForPlayer(player) && !memberForPlayer(player)?.owner"
                      type="button"
                      :disabled="game.isLoading.value"
                      @click="removeWaitingPlayer(memberForPlayer(player)!.userId)"
                    >
                      移除
                    </button>
                  </span>
                </div>
                <div class="result-actions">
                  <button
                    v-if="isRoomOwner"
                    class="ghost-button"
                    type="button"
                    :disabled="game.isLoading.value"
                    @click="dissolveWaitingRoom"
                  >
                    解散
                  </button>
                  <button
                    v-else
                    class="ghost-button"
                    type="button"
                    :disabled="game.isLoading.value"
                    @click="leaveWaitingRoom"
                  >
                    離開
                  </button>
                  <button
                    v-if="isRoomOwner"
                    class="primary-button"
                    type="button"
                    :disabled="game.isLoading.value || !canStartOnlineRoom"
                    @click="startOnlineRoom"
                  >
                    開始遊戲 <span>→</span>
                  </button>
                  <button
                    v-else
                    class="primary-button"
                    type="button"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="game.toggleReady()"
                  >
                    {{ currentMember?.ready ? '取消準備' : '準備' }} <span>→</span>
                  </button>
                </div>
                <button
                  v-if="onlineMetadata?.access === 'private' && isRoomOwner && game.invitation.value"
                  class="invite-link"
                  type="button"
                  @click="copyInviteLink"
                >
                  複製邀請連結
                </button>
            </template>
          </WaitingRoomLayout>

        </section>

        <aside class="game-sidebar" :class="{ finished: gameFinished }">
          <section v-if="gameFinished" class="result-panel">
            <h2>{{ gameResultText }}</h2>
            <p>{{ firstTurnText }}，本局已結束。</p>
            <div class="result-actions">
              <button class="primary-button" type="button" :disabled="game.isLoading.value" @click="restartGame">
                返回房間 <span>→</span>
              </button>
            </div>
          </section>

          <section class="event-panel" :class="{ expanded: eventExpanded }">
            <div class="panel-title">
              <h2>戰局紀錄 <button v-if="roomWaiting && game.savableReplay.value" class="ghost-button" type="button" :disabled="replaySaving" @click="saveCurrentReplay">{{ replaySaved ? '已儲存' : '儲存本局' }}</button></h2>
              <button class="event-expand-button" type="button" @click="eventExpanded = !eventExpanded">
                {{ eventExpanded ? '收合' : '完整紀錄' }}
              </button>
            </div>
            <p v-if="replayError" class="form-error" role="alert">
              {{ replayError }}
              <button class="ghost-button" type="button" :disabled="replaySaving" @click="saveCurrentReplay">重試</button>
            </p>
            <ol class="event-feed">
              <li v-for="event in visibleEvents" :key="event.id">
                <i />
                <div
                  :role="event.eventType === 'EnabledRules' ? 'region' : undefined"
                  :aria-label="event.eventType === 'EnabledRules' ? '啟用規則' : undefined"
                >
                  <span>{{ event.title }}</span>
                  <p>{{ event.summary }}</p>
                </div>
              </li>
            </ol>
          </section>
        </aside>
      </div>
  </main>
</template>

<script setup lang="ts">
import type {
  CardInstanceId,
  Element,
  PlayableAction,
  PlayerId,
  PublicCard,
  PublicCardRefs,
  PublicGameEvent,
  PublicGameState,
  SecretStrategy,
  SecretStrategyOption,
  SpiritKind,
  StarKind,
  TeamId,
  ViewerId,
} from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '#shared/game-room'
import { createRuleModulePolicy, presentationForRuleModule } from '#shared/utils/rule-modules'
import { buildCardComposition, CARD_LEVELS } from '~/lib/card-composition'
import { cardElementGlyph } from '~/lib/card-face-presentation'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import { presentPersistentEffects } from '~/lib/persistent-effect-presentation'
import { presentDirectSecretStrategyAction, presentDiscardRetrievalAction, presentPlayableAction, presentSecretStrategyOption } from '~/lib/action-detail-presentation'
import { splitEarthChoiceKey, usesSplitEarthFormationGroups } from '#shared/utils/split-earth-formation-choice'
import { roomRouteResult } from '~/lib/navigation'
import { cardChoiceDraftCount, chainChoiceAnswer, isImmediateCardChoice, toggleChoiceCard } from '~/lib/pending-choice-interaction'
import { secretStrategyDraftAction } from '~/lib/secret-strategy-draft'
import { isLegalChainTrigger, sheepReturnCards } from '~/lib/pouch-choice'
import { useLayoutNotifications } from '~/lib/player-notifications-context'
import { useRulesCatalog } from '~/lib/rules-catalog'
import { presentApiError } from '~/lib/api-error-presentation'
import { abilityLevelPicker } from '~/lib/ability-level-picker'
import {
  completeVirtualFormationCardOffer,
  isVirtualFormationCardOffer,
  professionAbilityOfferKey,
  type VirtualFormationCardOffer,
} from '~/lib/profession-ability-input'

const HAND_SHORTCUT_KEYS = ['1', '2', '3', '4', '5'] as const
const ABILITY_SHORTCUT_KEYS = [...'qwertyuiop'] as const
const ACTION_SHORTCUT_KEYS = [...'asdfghjkl'] as const

function shortcutLabel(keys: readonly string[], index: number) {
  return keys[index]?.toUpperCase()
}

const route = useRoute()
const router = useRouter()
const session = usePlayerSession()
const notifications = useLayoutNotifications()
const rulesCatalog = useRulesCatalog()
const ruleModulePolicy = computed(() => createRuleModulePolicy(rulesCatalog.catalog.value?.ruleModules ?? []))
const routeReady = ref(false)
const roomRouteError = ref('')
const roomCode = ref('')
const replaySaving = ref(false)
const replaySaved = ref(false)
const replayError = ref('')
const battlefield = ref<HTMLElement | null>(null)

const viewer = ref<ViewerId>('observer')
const game = useGameRoom(viewer)
const state = game.state
const splendorPicker = computed(() => abilityLevelPicker(
  game.playableAbilities.value,
  'useSpiritSkill',
  'Splendor',
))
const darkSpiritPicker = computed(() => abilityLevelPicker(
  game.playableAbilities.value,
  'activateProfessionAbility',
  'dark:dark-spirit',
))
const directPlayableAbilities = computed(() => (
  game.playableAbilities.value.filter(
    ability => (ability.type !== 'useSpiritSkill' || ability.id !== 'Splendor')
      && (ability.type !== 'activateProfessionAbility' || ability.id !== 'dark:dark-spirit'),
  )
))
const ownDeckCards = computed<PublicCard[]>(() => {
  if (viewer.value === 'observer') return []
  const cards = state.value.playerDecks.find(entry => entry.player === viewer.value)?.cards
  return cards?.kind === 'known' ? cards.cards : []
})

function chooseInitialPouchCard(card: CardInstanceId | undefined) {
  if (card !== undefined) game.chooseInitialPouch(card)
}
type PouchStrategyAction = {
  label: string
  detail: string
  strategy: SecretStrategy
  requirement: SecretStrategyOption
}
const pouchStrategyActions = computed<PouchStrategyAction[]>(() => {
  if (viewer.value === 'observer') return []
  const pouch = state.value.pouches.find(entry => entry.owner === viewer.value)?.card
  if (!pouch) return []
  const strategies = new Set<SecretStrategy>()
  const requirements = game.playableSecretStrategies.value
    .filter(requirement => requirement.sourceCard === pouch.id)
  return requirements.flatMap((requirement) => {
    if (strategies.has(requirement.strategy)
      || (requirement.input === 'deckDiscardSwap'
        && requirement.discardCards.length < requirement.requiredCardCount)) return []

    strategies.add(requirement.strategy)
    return [{
      label: `秘計‧${strategyLabel(requirement.strategy)}`,
      detail: presentDirectSecretStrategyAction(requirement),
      strategy: requirement.strategy,
      requirement,
    }]
  })
})
const directAbilityShortcutOffset = computed(() => pouchStrategyActions.value.length)
const darkSpiritShortcutIndex = computed(() => (
  directAbilityShortcutOffset.value + directPlayableAbilities.value.length
))
const splendorShortcutIndex = computed(() => (
  darkSpiritShortcutIndex.value + Number(Boolean(darkSpiritPicker.value))
))
const discardRetrievalShortcutIndex = computed(() => (
  splendorShortcutIndex.value + Number(Boolean(splendorPicker.value))
))
const pouchChoiceKind = ref<'chain' | 'sheep' | null>(null)
const secretStrategyDraft = ref<SecretStrategyOption | null>(null)
const secretStrategyTargetSelection = ref<PlayerId | null>(null)
const secretStrategyStarSelection = ref<StarKind | null>(null)
const secretStrategyBreakStar = ref(false)
const secretStrategyRetreatSelection = ref<CardInstanceId | 'clearEnvironment' | null>(null)
const pouchDeckSelection = ref<number[]>([])
const pouchDiscardSelection = ref<number[]>([])
const pouchOwnerSelection = ref<PlayerId | null>(null)
const chainStrategySelection = ref<SecretStrategy | null>(null)
const strategyTargetSelection = ref<PlayerId | null>(null)
const strategyStarSelection = ref<StarKind | null>(null)
const strategyBreakStar = ref(false)
const strategyDiscardCard = ref<number | null>(null)
const pouchSwapRequiredCount = ref(0)
const pouchSwapDeckCards = ref<number[]>([])
const pouchSwapDiscardCards = ref<number[]>([])
const ownDiscardCards = computed(() => {
  if (viewer.value === 'observer') return []
  return state.value.playerDiscards.find(entry => entry.player === viewer.value)?.cards ?? []
})
const ownHandCards = computed(() => {
  if (viewer.value === 'observer') return []
  const hand = state.value.hands.find(entry => entry.player === viewer.value)?.cards
  return hand?.kind === 'known' ? hand.cards : []
})
const chainPouchOwners = computed(() => {
  const choice = state.value.pendingChoice
  return choice?.visibility === 'visible' && choice.choice.type === 'chain'
    ? choice.choice.pouchOwners
    : []
})
const chainPouchCard = computed(() => (
  pouchChoiceKind.value === 'chain' && pouchDeckSelection.value.length >= 1
    ? chainPouchCards.value.find(card => card.id === pouchDeckSelection.value[0]) ?? null
    : null
))
const chainTriggerCard = computed(() => (
  pouchChoiceKind.value === 'chain' && pouchDeckSelection.value.length === 2
    ? chainPouchCards.value.find(card => card.id === pouchDeckSelection.value[1]) ?? null
    : null
))
const chainPouchCards = computed(() => (
  (state.value.pendingChoice?.visibility === 'visible'
    && state.value.pendingChoice.choice.type === 'chain'
    ? state.value.pendingChoice.choice.deckCards
    : ownDeckCards.value
  )
))
const chainTriggerCards = computed(() => (
  chainPouchCards.value.filter(card => (
    isLegalChainTrigger(chainPouchCard.value, card)
  ))
))
const pouchSwapSelectableDeckCards = computed(() => {
  const choice = state.value.pendingChoice
  const cards = choice?.visibility === 'visible' && choice.choice.type === 'sheepStealing'
    ? choice.choice.deckCards
    : ownDeckCards.value
  return cards.filter(card => pouchSwapDeckCards.value.includes(card.id))
})
const pouchSwapReturnCards = computed(() => sheepReturnCards(
  pouchSwapDiscardCards.value,
  ownDiscardCards.value,
  ownDeckCards.value,
  pouchDeckSelection.value,
))
const chainStrategyOptions = computed(() => {
  const card = chainTriggerCard.value
  if (!card) return []
  const choice = state.value.pendingChoice
  if (choice?.visibility !== 'visible' || choice.choice.type !== 'chain') return []
  const prospectiveDeckCount = ownDeckCards.value.length - 2
  const sheepCanComplete = prospectiveDeckCount + ownDiscardCards.value.length >= 2
  return choice.choice.strategyOptions
    .filter(option => option.sourceCard === card.id)
    .filter(option => option.strategy !== 'SheepStealing' || sheepCanComplete)
})
const selectedChainStrategyAction = computed(() => chainStrategyOptions.value.find(
  option => option.strategy === chainStrategySelection.value,
) ?? null)
const secretStrategyAction = computed(() => {
  const draft = secretStrategyDraft.value
  return draft
    ? secretStrategyDraftAction(draft, {
        targetPlayer: secretStrategyTargetSelection.value,
        star: secretStrategyStarSelection.value,
        breakStar: secretStrategyBreakStar.value,
        retreat: secretStrategyRetreatSelection.value,
      })
    : undefined
})
const canSubmitSecretStrategyDraft = computed(() => secretStrategyAction.value !== undefined)
const canSubmitPouchChoice = computed(() => {
  if (pouchChoiceKind.value === 'sheep') {
    const count = pouchSwapRequiredCount.value
    return count > 0
      && pouchDeckSelection.value.length === count
      && pouchDiscardSelection.value.length === count
      && containsSelectedCards(pouchSwapSelectableDeckCards.value, pouchDeckSelection.value)
      && containsSelectedCards(pouchSwapReturnCards.value, pouchDiscardSelection.value)
  }
  if (pouchChoiceKind.value !== 'chain'
    || !pouchOwnerSelection.value
    || pouchDeckSelection.value.length < 1
    || pouchDeckSelection.value.length > 2) return false
  if (pouchDeckSelection.value.length === 1) return true
  const requirement = selectedChainStrategyAction.value
  if (!requirement) return false
  if (requirement.input === 'targetPlayer') return !!strategyTargetSelection.value
  if (requirement.input === 'star') return !!strategyStarSelection.value
  if (requirement.input === 'deckDiscardSwap') return true
  return true
})

function containsSelectedCards(cards: PublicCard[], selected: CardInstanceId[]): boolean {
  const candidateIds = new Set(cards.map(card => card.id))
  return selected.every(card => candidateIds.has(card))
}

function resetPouchChoice() {
  pouchChoiceKind.value = null
  pouchDeckSelection.value = []
  pouchDiscardSelection.value = []
  pouchOwnerSelection.value = null
  chainStrategySelection.value = null
  strategyTargetSelection.value = null
  strategyStarSelection.value = null
  strategyBreakStar.value = false
  strategyDiscardCard.value = null
  pouchSwapRequiredCount.value = 0
  pouchSwapDeckCards.value = []
  pouchSwapDiscardCards.value = []
}

function resetSecretStrategyDraft() {
  secretStrategyDraft.value = null
  secretStrategyTargetSelection.value = null
  secretStrategyStarSelection.value = null
  secretStrategyBreakStar.value = false
  secretStrategyRetreatSelection.value = null
}

function togglePouchCard(
  kind: 'pouchDeck' | 'pouchDiscard',
  card: number,
  maximum: number,
) {
  const selection = {
    pouchDeck: pouchDeckSelection,
    pouchDiscard: pouchDiscardSelection,
  }[kind]
  selection.value = toggleChoiceCard(selection.value, card, maximum)

  if (kind === 'pouchDeck') {
    const available = new Set(pouchSwapReturnCards.value.map(candidate => candidate.id))
    pouchDiscardSelection.value = pouchDiscardSelection.value.filter(id => available.has(id))
  }
}

function chooseChainPouchCard(card: CardInstanceId) {
  pouchDeckSelection.value = [card]
  resetChainStrategyChoice()
}

function chooseChainTriggerCard(card: CardInstanceId) {
  const pouch = pouchDeckSelection.value[0]
  if (pouch === undefined) return

  pouchDeckSelection.value = [pouch, card]
  resetChainStrategyChoice()
}

function clearChainPouchCard() {
  pouchDeckSelection.value = []
  resetChainStrategyChoice()
}

function clearChainTriggerCard() {
  pouchDeckSelection.value = pouchDeckSelection.value.slice(0, 1)
  resetChainStrategyChoice()
}

function resetChainStrategyChoice() {
  chainStrategySelection.value = null
  strategyTargetSelection.value = null
  strategyStarSelection.value = null
  strategyBreakStar.value = false
  strategyDiscardCard.value = null
}

function startPouchAction(action: PouchStrategyAction) {
  if (action.requirement.input === 'none' || action.requirement.input === 'deckDiscardSwap') {
    void game.triggerSecretStrategy(action.strategy)
    return
  }

  resetSecretStrategyDraft()
  secretStrategyDraft.value = action.requirement
}

async function submitSecretStrategyDraft() {
  const action = secretStrategyAction.value
  if (!action) return

  if (await game.triggerSecretStrategy(action.strategy, action.options)) {
    resetSecretStrategyDraft()
  }
}

function startPlayableAction(action: PlayableAction) {
  if (action.type === 'performFormation' && action.id === 'pouch:chain') {
    resetPouchChoice()
    void game.performPlayableAction(action).then((submitted) => {
      if (submitted) {
        pouchChoiceKind.value = 'chain'
        pouchOwnerSelection.value = null
      }
    })
    return
  }
  void game.performPlayableAction(action)
}

async function submitPouchChoice() {
  if (!canSubmitPouchChoice.value) return
  if (pouchChoiceKind.value === 'sheep') {
    const submitted = await game.answerSheepStealingChoice(
      pouchDeckSelection.value,
      pouchDiscardSelection.value,
    )
    if (submitted !== false) resetPouchChoice()
    return
  }
  const owner = pouchOwnerSelection.value
  const pouchCard = pouchDeckSelection.value[0]
  if (!owner || pouchCard === undefined) return
  const submitted = await game.answerChainChoice(chainChoiceAnswer({
    pouchOwner: owner,
    pouchCard,
    triggerCard: pouchDeckSelection.value[1],
    strategy: chainStrategySelection.value ?? undefined,
    targetPlayer: strategyTargetSelection.value ?? undefined,
    star: strategyStarSelection.value ?? undefined,
    breakStar: strategyBreakStar.value,
    discardCard: strategyDiscardCard.value ?? undefined,
  }))
  const nextChoice = state.value.pendingChoice
  if (
    submitted
    && (nextChoice?.visibility !== 'visible' || nextChoice.choice.type !== 'sheepStealing')
  ) resetPouchChoice()
}

watch(
  [
    () => state.value.pendingChoice,
    () => game.pendingChoiceDraftEpoch.value,
  ],
  ([choice, draftEpoch], [, previousDraftEpoch]) => {
    if (draftEpoch !== previousDraftEpoch) resetPouchChoice()
    if (!choice || choice.visibility !== 'visible' || viewer.value !== choice.player) {
      if (pouchChoiceKind.value) resetPouchChoice()
      return
    }
    if (choice.choice.type === 'chain') {
      pouchChoiceKind.value = 'chain'
      pouchOwnerSelection.value ??= choice.choice.pouchOwners[0] ?? null
      return
    }
    if (choice.choice.type === 'sheepStealing') {
      pouchChoiceKind.value = 'sheep'
      pouchDeckSelection.value = []
      pouchDiscardSelection.value = []
      pouchSwapRequiredCount.value = 2
      pouchSwapDeckCards.value = choice.choice.deckCards.map(card => card.id)
      pouchSwapDiscardCards.value = [
        ...choice.choice.discardCards.map(card => card.id),
        ...choice.choice.deckCards.map(card => card.id),
      ]
    }
  },
)

watch(
  [
    viewer,
    () => state.value.currentPlayer,
    () => state.value.pendingChoice,
    () => game.connectionState.value,
    () => game.playableSecretStrategies.value,
  ],
  () => {
    if (secretStrategyDraft.value) resetSecretStrategyDraft()
  },
)

const actionDetail = ref<string | null>(null)
const discardRetrievalDetail = computed(() => {
  const action = game.playableDiscardRetrieval.value
  return action?.detail ? presentDiscardRetrievalAction(action) : ''
})
const splendorMenuOpen = ref(false)
const splendorMenuTrigger = ref<HTMLButtonElement | null>(null)
const darkSpiritMenuOpen = ref(false)
const darkSpiritMenuTrigger = ref<HTMLButtonElement | null>(null)
const virtualFormationCardDraft = ref<VirtualFormationCardOffer | null>(null)
const virtualFormationCardDialog = ref<HTMLElement | null>(null)
let virtualFormationCardReturnFocus: HTMLElement | null = null
const eventExpanded = ref(false)
const showSetupReveal = ref(false)
const discardOpen = ref(false)
const activeDiscardOwner = ref<PlayerId | null>(null)
const discardTrigger = ref<HTMLButtonElement | null>(null)

watch(
  [
    viewer,
    () => state.value.currentPlayer,
    () => state.value.turnNumber,
    () => state.value.phase,
    () => game.connectionState.value,
    () => game.playableAbilities.value
      .filter(ability => ability.type === 'activateProfessionAbility')
      .map(professionAbilityOfferKey)
      .join('|'),
  ],
  () => {
    const draft = virtualFormationCardDraft.value
    if (!draft) return

    const draftKey = professionAbilityOfferKey(draft)
    const stillOffered = game.playableAbilities.value.some(ability => (
      ability.type === 'activateProfessionAbility'
      && professionAbilityOfferKey(ability) === draftKey
    ))
    if (
      game.connectionState.value !== 'connected'
      || viewer.value !== state.value.currentPlayer
      || state.value.status !== 'InProgress'
      || state.value.phase !== 'ActiveEffects'
      || Boolean(state.value.pendingChoice)
      || draft.cards.length !== game.selectedCards.value.length
      || draft.cards.some(card => !game.selectedCards.value.includes(card))
      || !stillOffered
    ) {
      closeVirtualFormationCardDraft(false)
    }
  },
)

const ruleGroupLabels = {
  optional: '選用規則',
  advanced: '進階規則',
  theme: '主題規則',
}
const ruleGroups = computed(() => (['optional', 'advanced', 'theme'] as const).map(id => ({
  id,
  label: ruleGroupLabels[id],
  rules: ruleModulePolicy.value.modules
    .map(module => ({
      id: module.id,
      ...presentationForRuleModule(module.id),
      group: module.category,
    }))
    .filter(module => module.group === id),
})))
const ruleLabelById = computed(() => new Map<string, string>(
  ruleGroups.value.flatMap(group => group.rules).map(rule => [rule.id, rule.label]),
))
const onlineMetadata = computed(() => game.metadata.value)
const activeRoomName = computed(() => onlineMetadata.value?.name ?? '')
const waitingRoomSummary = computed(() => game.invitation.value
  ? `房號 ${game.invitation.value.roomCode} · ${waitingStatusText.value}`
  : waitingStatusText.value)
const roomWaiting = computed(() => onlineMetadata.value?.status === 'Waiting')
const gameFinished = computed(() => state.value.status === 'Finished')
const firstPlayer = computed<PlayerId | null>(() => (
  roomWaiting.value ? null : state.value.turnOrder[0] ?? null
))
const firstTurnText = computed(() => firstPlayer.value ? `${playerLabel(firstPlayer.value)} 先手` : '尚未決定先手')
const onlinePlayers = computed(() => onlineMetadata.value?.players ?? [])
const waitingStatusText = computed(() => `${onlineMetadata.value?.members.length ?? 0} / ${onlineMetadata.value?.players.length ?? 0} 玩家`)
const currentMember = computed(() => onlineMetadata.value?.members.find(
  (member) => member.userId === session.userId.value,
))
const ownPlayer = computed(() => currentMember.value?.player ?? '')
const isRoomOwner = computed(() => currentMember.value?.owner === true)
const roomConnected = computed(() => game.connectionState.value === 'connected')
const enabledRuleLabels = computed(() => [
  '基礎規則',
  ...(onlineMetadata.value?.enabledRuleModules ?? [])
    .flatMap(moduleId => {
      const label = ruleLabelById.value.get(moduleId)
      return label ? [label] : []
    }),
])
const visibleEvents = computed<PublicGameEvent[]>(() => roomWaiting.value
  ? game.publicEvents.value
  : [...game.publicEvents.value, {
      id: 'enabled-rules',
      eventType: 'EnabledRules',
      title: '啟用規則',
      summary: enabledRuleLabels.value.join(' · '),
    }])
const canStartOnlineRoom = computed(() => {
  const metadata = onlineMetadata.value

  return Boolean(
    metadata
    && roomConnected.value
    && metadata.members.length === metadata.capacity
    && metadata.members.every((member) => member.connected)
    && metadata.members.every((member) => member.owner || member.ready),
  )
})
type SeatPosition = 'top' | 'right' | 'bottom' | 'left'

interface PlayerSeat {
  player: PlayerId
  position: SeatPosition
}

const playerSeats = computed<PlayerSeat[]>(() => {
  const order = state.value.turnOrder
  const ownIndex = order.indexOf(ownPlayer.value)
  const startIndex = ownIndex >= 0 ? ownIndex : 0
  const relativeOrder = [...order.slice(startIndex), ...order.slice(0, startIndex)]
  const positions: SeatPosition[] = relativeOrder.length === 4
    ? ['bottom', 'left', 'top', 'right']
    : ['bottom', 'top']

  return relativeOrder.map((player, index) => ({
    player,
    position: positions[index] ?? 'top',
  }))
})

const previousFormationCards = computed(() => (
  cardTokensForRefs(state.value.previousTurnFormation?.cards, 'previous-formation')
))
const activeDiscardCards = computed(() => {
  if (!state.value.playerDiscards.length) return state.value.discard
  const owner = activeDiscardOwner.value
    ?? (ownPlayer.value || undefined)
    ?? state.value.currentPlayer
    ?? state.value.playerDiscards[0]?.player
  return state.value.playerDiscards.find(pile => pile.player === owner)?.cards ?? []
})
const visibleDiscardPiles = computed(() => (
  state.value.playerDiscards.length
    ? state.value.playerDiscards.map(pile => ({
        owner: pile.player,
        cards: pile.cards,
        position: playerSeats.value.find(seat => seat.player === pile.player)?.position ?? 'top',
      }))
    : [{ owner: null, cards: state.value.discard, position: null }]
))
const discardComposition = computed(() => buildCardComposition(activeDiscardCards.value))
const activeTeams = computed(() => [...new Set(state.value.players.map((player) => player.team))])
const showSkip = computed(() => (
  roomConnected.value
  && game.playablePass.value !== null
))
const gameResultText = computed(() => {
  if (state.value.fiveStarAlignment) {
    return `五星連珠 · ${teamLabel(state.value.fiveStarAlignment.team)} 勝利`
  }

  if (state.value.winnerTeam) {
    return `${teamLabel(state.value.winnerTeam)} 勝利`
  }

  const aliveTeams = state.value.hp.filter((entry) => entry.hp > 0)

  if (aliveTeams.length === 1) {
    return `${teamLabel(aliveTeams[0]!.team)} 勝利`
  }

  return '戰局結束'
})
let actionDetailTimer: ReturnType<typeof setTimeout> | undefined
let setupRevealTimer: ReturnType<typeof setTimeout> | undefined
interface CardToken {
  id: string
  cardId: number
  card: PublicCard | null
  label: string
  element?: Element | null
  level?: number | null
  hidden: boolean
  selectable: boolean
  selected: boolean
}

function leaveGame() {
  game.clearRoom()
  void router.push('/rooms')
}

function resetRoomRouteState() {
  clearTimeout(actionDetailTimer)
  clearTimeout(setupRevealTimer)
  roomCode.value = ''
  viewer.value = 'observer'
  replaySaving.value = false
  replaySaved.value = false
  replayError.value = ''
  resetPouchChoice()
  resetSecretStrategyDraft()
  actionDetail.value = null
  splendorMenuOpen.value = false
  splendorMenuTrigger.value = null
  darkSpiritMenuOpen.value = false
  darkSpiritMenuTrigger.value = null
  virtualFormationCardDraft.value = null
  virtualFormationCardDialog.value = null
  virtualFormationCardReturnFocus = null
  eventExpanded.value = false
  showSetupReveal.value = false
  discardOpen.value = false
  activeDiscardOwner.value = null
  discardTrigger.value = null
}

function discardPileUnavailable(cards: readonly unknown[]): boolean {
  return cards.length === 0 || Boolean(state.value.pendingChoice)
}

function toggleDiscardComposition(owner: PlayerId | null, trigger: HTMLButtonElement) {
  if (discardOpen.value && activeDiscardOwner.value === owner) {
    closeDiscardComposition()
    return
  }

  const pile = visibleDiscardPiles.value.find(candidate => candidate.owner === owner)
  if (!pile || discardPileUnavailable(pile.cards)) {
    return
  }

  discardTrigger.value = trigger
  activeDiscardOwner.value = owner
  discardOpen.value = true
}

function closeDiscardComposition() {
  if (!discardOpen.value) {
    return
  }

  discardOpen.value = false
  void nextTick(() => discardTrigger.value?.focus())
}

function closeSplendorMenu(returnFocus = false) {
  if (!splendorMenuOpen.value) {
    return
  }

  splendorMenuOpen.value = false
  if (returnFocus) {
    void nextTick(() => splendorMenuTrigger.value?.focus())
  }
}

function closeDarkSpiritMenu(returnFocus = false) {
  if (!darkSpiritMenuOpen.value) {
    return
  }

  darkSpiritMenuOpen.value = false
  if (returnFocus) {
    void nextTick(() => darkSpiritMenuTrigger.value?.focus())
  }
}

function startSplendorAction(ability: PlayableAction) {
  closeSplendorMenu(true)
  void game.performPlayableAction(ability)
}

function startDarkSpiritAction(ability: PlayableAction) {
  closeDarkSpiritMenu(true)
  void game.performPlayableAction(ability)
}

function startDirectAbility(
  ability: Extract<PlayableAction, { type: 'activateProfessionAbility' | 'useSpiritSkill' }>,
) {
  if (ability.type !== 'activateProfessionAbility' || !isVirtualFormationCardOffer(ability)) {
    void game.performPlayableAction(ability)
    return
  }

  virtualFormationCardReturnFocus = document.activeElement instanceof HTMLElement
    ? document.activeElement
    : null
  virtualFormationCardDraft.value = ability
  void nextTick(() => {
    virtualFormationCardDialog.value
      ?.querySelector<HTMLButtonElement>('.virtual-formation-card-option')
      ?.focus()
  })
}

function closeVirtualFormationCardDraft(returnFocus = true) {
  if (!virtualFormationCardDraft.value) {
    return
  }

  const focusTarget = virtualFormationCardReturnFocus
  virtualFormationCardDraft.value = null
  virtualFormationCardReturnFocus = null
  if (returnFocus) {
    void nextTick(() => focusTarget?.focus())
  }
}

function chooseVirtualFormationCard(element: Element, level: number) {
  const draft = virtualFormationCardDraft.value
  if (!draft || game.isLoading.value || !roomConnected.value) {
    return
  }
  const completed = completeVirtualFormationCardOffer(draft, element, level)
  if (!completed) {
    return
  }

  closeVirtualFormationCardDraft(false)
  void game.performPlayableAction(completed)
}

function handlePageClick() {
  closeDiscardComposition()
  closeSplendorMenu()
  closeDarkSpiritMenu()
}

function targetsEditableControl(target: EventTarget | null) {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || (target instanceof HTMLElement && target.isContentEditable)
}

function shortcutHasModifier(event: KeyboardEvent) {
  return event.metaKey || event.ctrlKey || event.altKey || event.shiftKey
}

function hasKeyboardBlockingLayer() {
  return Boolean(
    discardOpen.value
    || virtualFormationCardDraft.value
    || darkSpiritMenuOpen.value
    || splendorMenuOpen.value
    || secretStrategyDraft.value
    || pouchChoiceKind.value
    || state.value.pendingChoice
    || state.value.pendingRandomness
    || showSetupReveal.value,
  )
}

function isMainShortcutContext() {
  return onlineMetadata.value?.status === 'Active'
    && state.value.status === 'InProgress'
    && state.value.phase === 'ActiveEffects'
    && viewer.value === state.value.currentPlayer
    && roomConnected.value
    && !hasKeyboardBlockingLayer()
}

function canUseHandShortcut() {
  return isMainShortcutContext()
    && !game.isSubmittingCommand.value
    && (!game.isLoading.value || game.isQueryingPlayableActions.value)
}

function canUseCommandShortcut() {
  return isMainShortcutContext()
    && !game.isLoading.value
    && !game.isSubmittingCommand.value
    && !game.isQueryingPlayableActions.value
}

function shortcutButton(group: 'ability' | 'action', index: number) {
  const buttons = battlefield.value?.querySelectorAll<HTMLButtonElement>(
    `button[data-keyboard-shortcut="${group}"]`,
  )
  if (!buttons) return undefined

  return Array.from(buttons)
    .filter(button => !button.disabled && button.getClientRects().length > 0)[index]
}

function randomIndex(length: number) {
  const uint32Range = 2 ** 32
  const acceptedRange = uint32Range - (uint32Range % length)
  const sample = new Uint32Array(1)
  let value: number

  do {
    crypto.getRandomValues(sample)
    value = sample[0]!
  } while (value >= acceptedRange)

  return value % length
}

function triggerKeyboardShortcut(event: KeyboardEvent) {
  if (event.repeat || shortcutHasModifier(event) || targetsEditableControl(event.target)) {
    return false
  }

  const key = event.key.toLowerCase()
  if (
    key === 'r'
    && state.value.status === 'Preparing'
    && game.interaction.value.canChooseInitialPouch
    && viewer.value !== 'observer'
    && roomConnected.value
    && !game.isLoading.value
    && ownDeckCards.value.length > 0
  ) {
    event.preventDefault()
    const card = ownDeckCards.value[randomIndex(ownDeckCards.value.length)]!
    void game.chooseInitialPouch(card.id)
    return true
  }

  const handIndex = HAND_SHORTCUT_KEYS.indexOf(key as typeof HAND_SHORTCUT_KEYS[number])
  if (handIndex >= 0 && canUseHandShortcut()) {
    const card = ownHandCards.value[handIndex]
    if (card && viewer.value !== 'observer') {
      event.preventDefault()
      game.toggleCardSelection(viewer.value, card.id)
      return true
    }
  }

  if (!canUseCommandShortcut()) return false

  const abilityIndex = ABILITY_SHORTCUT_KEYS.indexOf(key as typeof ABILITY_SHORTCUT_KEYS[number])
  if (abilityIndex >= 0) {
    const button = shortcutButton('ability', abilityIndex)
    if (button) {
      event.preventDefault()
      button.click()
      return true
    }
  }

  const actionIndex = ACTION_SHORTCUT_KEYS.indexOf(key as typeof ACTION_SHORTCUT_KEYS[number])
  if (actionIndex >= 0) {
    const button = shortcutButton('action', actionIndex)
    if (button) {
      event.preventDefault()
      button.click()
      return true
    }
  }

  return false
}

function handlePageKeydown(event: KeyboardEvent) {
  if (
    event.metaKey
    && !event.ctrlKey
    && !event.altKey
    && !event.shiftKey
    && !event.repeat
    && event.key.toLowerCase() === 'k'
    && !targetsEditableControl(event.target)
    && onlineMetadata.value?.status === 'Active'
    && state.value.status === 'InProgress'
    && state.value.phase === 'ActiveEffects'
    && viewer.value === state.value.currentPlayer
    && !hasKeyboardBlockingLayer()
    && !game.isLoading.value
    && !game.isSubmittingCommand.value
    && !game.isQueryingPlayableActions.value
  ) {
    event.preventDefault()
    void game.advanceAutomatic()
    return
  }

  if (triggerKeyboardShortcut(event)) {
    return
  }

  if (event.key !== 'Escape') {
    return
  }

  if (secretStrategyDraft.value) {
    event.preventDefault()
    resetSecretStrategyDraft()
    return
  }

  if (virtualFormationCardDraft.value) {
    event.preventDefault()
    closeVirtualFormationCardDraft()
    return
  }

  if (darkSpiritMenuOpen.value) {
    event.preventDefault()
    closeDarkSpiritMenu(true)
    return
  }

  if (splendorMenuOpen.value) {
    event.preventDefault()
    closeSplendorMenu(true)
    return
  }

  if (discardOpen.value) {
    event.preventDefault()
    closeDiscardComposition()
    return
  }

}



async function restartGame() {
  await game.resetOnlineRoom()
  // Reset replay save state when returning to the room for a new game
  replaySaved.value = false
  replayError.value = ''
}

async function updateWaitingRuleModules(next: string[]) {
  await game.updateRuleModules(next)
}

async function startOnlineRoom() {
  await game.startOnlineGame()
}

async function leaveWaitingRoom() {
  if (await game.leaveOnlineRoom()) await router.replace('/rooms')
}

async function removeWaitingPlayer(userId: string) {
  await game.removeOnlinePlayer(userId)
}

async function dissolveWaitingRoom() {
  if (await game.dissolveOnlineRoom()) await router.replace('/rooms')
}

async function copyInviteLink() {
  const invitation = game.invitation.value
  if (!invitation) return
  const url = new URL(`/rooms/${encodeURIComponent(roomCode.value)}`, window.location.origin)
  url.searchParams.set('invite', invitation.inviteToken)
  await navigator.clipboard.writeText(url.toString())
}

async function saveCurrentReplay() {
  const sourceGameId = game.savableReplay.value?.sourceGameId
  if (!sourceGameId || replaySaved.value) return
  replaySaving.value = true
  try {
    await $fetch('/api/replays', { method: 'POST', body: { sourceGameId } })
    replaySaved.value = true
  } catch (error: unknown) {
    const code = (error as { data?: { data?: { code?: string } } }).data?.data?.code
    if (code === 'replayLibraryFull') {
      await router.push({ path: '/replays', query: { save: sourceGameId } })
    } else {
      replayError.value = presentApiError(error, '無法儲存本局')
    }
  } finally {
    replaySaving.value = false
  }
}

function returnToLobby() {
  void router.push('/rooms')
}

function applyRoomResponse(response: GameRoomResponse) {
  const ownMember = response.metadata.members.find(member => member.userId === session.userId.value)
  viewer.value = ownMember?.player ?? 'observer'
  roomCode.value = response.gameId
  eventExpanded.value = false
  notifications.dismissRoom(response.gameId)
  game.applyRoomResponse(response)
}

let roomLoadRevision = 0

async function loadRoomRoute() {
  const revision = ++roomLoadRevision
  const gameId = typeof route.params.gameId === 'string' ? route.params.gameId : ''
  resetRoomRouteState()
  routeReady.value = false
  roomRouteError.value = ''
  game.clearRoom()

  let response: GameRoomResponse
  try {
    response = await $fetch<GameRoomResponse>(`/api/games/${encodeURIComponent(gameId)}`)
  } catch {
    try {
      response = await $fetch<GameRoomResponse>(`/api/games/${encodeURIComponent(gameId)}/join`, {
        method: 'POST',
        body: { invite: typeof route.query.invite === 'string' ? route.query.invite : undefined },
      })
    } catch (error) {
      if (revision === roomLoadRevision) {
        roomRouteError.value = roomRouteResult(error)
        routeReady.value = true
      }
      return
    }
  }

  if (revision !== roomLoadRevision) return
  if (response.metadata.status === 'Dissolved') {
    roomRouteError.value = '找不到這個房間'
  } else {
    applyRoomResponse(response)
    if (route.query.invite) await router.replace(`/rooms/${encodeURIComponent(response.gameId)}`)
  }
  if (revision === roomLoadRevision) routeReady.value = true
}

onMounted(() => {
  void rulesCatalog.load().catch(() => undefined)
  window.addEventListener('click', handlePageClick)
  window.addEventListener('keydown', handlePageKeydown)
  watch(() => [route.params.gameId, route.query.invite], () => { void loadRoomRoute() }, { immediate: true })
})

onBeforeUnmount(() => {
  window.removeEventListener('click', handlePageClick)
  window.removeEventListener('keydown', handlePageKeydown)
  roomLoadRevision += 1
  clearTimeout(actionDetailTimer)
  clearTimeout(setupRevealTimer)
  game.clearRoom()
})

watch(
  () => notifications.notifications.value,
  (items) => {
    const removal = items.find(notification => notification.gameId === roomCode.value && (notification.kind === 'removed' || notification.kind === 'dissolved'))
    if (!removal) return
    notifications.dismiss(removal.id)
    game.clearRoom()
    void router.replace('/rooms')
  },
  { deep: true },
)

watch(
  () => state.value.discard.length + state.value.playerDiscards.reduce((total, pile) => total + pile.cards.length, 0),
  (length) => { if (length === 0) closeDiscardComposition() },
)
watch(() => state.value.pendingChoice, choice => { if (choice) closeDiscardComposition() })
watch(
  () => onlineMetadata.value?.status,
  (status, previous) => {
    if (status !== 'Active' || previous === 'Active') return
    clearTimeout(setupRevealTimer)
    showSetupReveal.value = true
    setupRevealTimer = setTimeout(() => { showSetupReveal.value = false }, 1800)
  },
)
watch(() => game.roomDissolved.value, dissolved => { if (dissolved) void router.replace('/rooms') })

function cardsFor(player: PlayerId): CardToken[] {
  const hand = state.value.hands.find((entry) => entry.player === player)
  if (!hand) return []

  if (hand.cards.kind === 'known') {
    return hand.cards.cards.map((card, index) => ({
      id: `${player}-known-${index}-${card.id}`,
      cardId: card.id,
      card,
      label: card.label,
      element: card.element,
      level: card.level,
      hidden: false,
      selectable: game.canSelectCard(player),
      selected: game.selectedCards.value.includes(card.id),
    }))
  }

  if (hand.cards.kind === 'partiallyKnown') {
    return hand.cards.cards.map((card, index) => card
      ? {
          id: `${player}-known-${index}-${card.id}`,
          cardId: card.id,
          card,
          label: card.label,
          element: card.element,
          level: card.level,
          hidden: false,
          selectable: false,
          selected: false,
        }
      : {
          id: `${player}-hidden-${index}`,
          cardId: -index - 1,
          card: null,
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: hand.cards.count }, (_, index) => ({
    id: `${player}-hidden-${index}`,
    cardId: -index - 1,
    card: null,
    label: '',
    hidden: true,
    selectable: false,
    selected: false,
  }))
}

function cardTokensForRefs(cards: PublicCardRefs | undefined, prefix: string): CardToken[] {
  if (!cards) return []

  if (cards.kind === 'known') {
    return cards.cards.map((card, index) => ({
      id: `${prefix}-known-${index}-${card.id}`,
      cardId: card.id,
      card,
      label: card.label,
      element: card.element,
      level: card.level,
      hidden: false,
      selectable: false,
      selected: false,
    }))
  }

  if (cards.kind === 'partiallyKnown') {
    return cards.cards.map((card, index) => card
      ? {
          id: `${prefix}-known-${index}-${card.id}`,
          cardId: card.id,
          card,
          label: card.label,
          element: card.element,
          level: card.level,
          hidden: false,
          selectable: false,
          selected: false,
        }
      : {
          id: `${prefix}-hidden-${index}`,
          cardId: -index - 1,
          card: null,
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: cards.count }, (_, index) => ({
    id: `${prefix}-hidden-${index}`,
    cardId: -index - 1,
    card: null,
    label: '',
    hidden: true,
    selectable: false,
    selected: false,
  }))
}

function handCount(player: PlayerId): number {
  const hand = state.value.hands.find((entry) => entry.player === player)
  if (!hand) return 0
  return hand.cards.kind === 'hidden' ? hand.cards.count : hand.cards.cards.length
}

function playerDeckCount(player: PlayerId): number {
  const pile = state.value.playerDecks.find(entry => entry.player === player)
  if (!pile) return 0
  return pile.cards.kind === 'hidden' ? pile.cards.count : pile.cards.cards.length
}

function playerDiscardCount(player: PlayerId): number {
  return state.value.playerDiscards.find(entry => entry.player === player)?.cards.length ?? 0
}

function teamStar(team: TeamId) {
  return state.value.teamStars.find(owned => owned.team === team)?.star
}

function starHistoryLabel(player: PlayerId): string {
  const stars = state.value.starHistories.find(history => history.player === player)?.stars ?? []
  if (!stars.length) return ''

  return stars.map(star => cardElementGlyph(star)).join('、')
}

function spiritFor(player: PlayerId) {
  return state.value.spirits.find(owned => owned.player === player)
}

function spiritLabel(spirit: SpiritKind): string {
  return {
    Metal: '金精靈',
    Wood: '木精靈',
    Water: '水精靈',
    Fire: '火精靈',
    Earth: '土精靈',
    Evil: '惡精靈',
    Death: '死精靈',
  }[spirit]
}

function professionFor(player: PlayerId) {
  return state.value.professions.find(profession => profession.player === player)
}

function persistentEffectsFor(player: PlayerId) {
  return presentPersistentEffects(state.value, player, teamForPlayer(player))
}

function professionSummaryLabel(player: PlayerId): string {
  const profession = professionFor(player)
  return profession
    ? `職業 ${profession.name}。能力：${profession.abilities.join('；')}`
    : ''
}

function starLabel(star: import('~/types/fewfc').StarKind): string {
  return {
    Metal: '金星‧太白',
    Wood: '木星‧歲星',
    Water: '水星‧辰星',
    Fire: '火星‧熒惑',
    Earth: '土星‧鎮星',
  }[star]
}

function strategyLabel(strategy: SecretStrategy): string {
  return {
    GoldenCicada: '金蟬',
    StealTheBeam: '偷梁',
    MuddyWaters: '混水',
    WatchTheFire: '觀火',
    LureTheTigerAway: '離山',
    ReturnSoul: '還魂',
    SheepStealing: '牽羊',
    DarkCrossing: '暗渡',
    DeceiveHeaven: '瞞天',
    Retreat: '走為',
  }[strategy]
}

function exposedDeckCards(player: PlayerId) {
  const cards = state.value.playerDecks.find(entry => entry.player === player)?.cards
  return cards?.kind === 'partiallyKnown' ? cards.cards.filter(card => card !== null) : []
}

function counterEffectsFor(player: PlayerId) {
  return state.value.counterEffects.filter((counter) => counter.owner === player)
}

function shieldFor(player: PlayerId): number {
  return state.value.shields.find((shield) => shield.player === player)?.value ?? 0
}

function playerConnected(player: PlayerId): boolean {
  if (player === ownPlayer.value) {
    return roomConnected.value
  }

  return memberForPlayer(player)?.connected ?? false
}

function teamHp(team: TeamId): number {
  return state.value.hp.find((entry) => entry.team === team)?.hp ?? 0
}

function teamLabel(team: TeamId): string {
  if (team === 'team-a') return 'A 隊'
  if (team === 'team-b') return 'B 隊'

  const player = state.value.players.find((candidate) => candidate.team === team)
  return player ? playerLabel(player.id) : team
}

function memberForPlayer(player: PlayerId): GameRoomMember | undefined {
  return onlineMetadata.value?.members.find((member) => member.player === player)
}

function playerLabel(value: PlayerId | null): string {
  if (!value) return '準備開始'
  return memberForPlayer(value)?.displayName ?? value
}

function playerInitialFor(player: PlayerId): string {
  return playerLabel(player).trim().charAt(0).toUpperCase() || '玩'
}

function teamForPlayer(player: PlayerId): TeamId {
  return state.value.players.find((candidate) => candidate.id === player)?.team ?? ''
}

function teamMembers(team: TeamId): string[] {
  return state.value.players
    .filter((player) => player.team === team)
    .map((player) => playerLabel(player.id))
}

function waitingMemberStatus(member: GameRoomMember | undefined): string {
  if (!member) return '等待中'
  if (!member.connected) return '未連線'
  if (member.owner) return '房主'
  return member.ready ? '已準備' : '未準備'
}

function playableActionName(action: PlayableAction): string {
  if (action.type === 'pass') {
    return '跳過'
  }
  if (action.type === 'retrievePreviousTurnDiscard') {
    return '棄牌回收'
  }
  if (action.type === 'triggerSecretStrategy') {
    return `秘計‧${strategyLabel(action.strategy)}`
  }
  if (action.type === 'changeProfession') {
    return `轉職：${action.name}`
  }
  if (action.type !== 'performFormation') {
    return action.name
  }
  if (action.matchOption) {
    return `${action.name}（${action.matchOption.preview ?? `指定牌 ${action.matchOption.card}`}）`
  }
  if (!action.starSubstitution) {
    return action.name
  }

  const substitution = action.starSubstitution
  const card = state.value.hands
    .flatMap(hand => hand.cards.kind === 'known' ? hand.cards.cards : [])
    .find(candidate => candidate.id === substitution.card)
  const cardLabel = card?.label ?? `牌 ${substitution.card}`

  return `${action.name}（${cardLabel}：${cardElementLabel(substitution.printedElement)}視為${cardElementLabel(substitution.interpretedElement)}）`
}

function cardElementLabel(element: Element): string {
  return `${environmentLabel(element).replace('環境', '')}牌`
}

function playableAbilityKey(
  ability: Extract<PlayableAction, { type: 'activateProfessionAbility' | 'useSpiritSkill' }>,
): string {
  if (ability.type === 'useSpiritSkill') {
    return `spirit:${ability.id}:${ability.selectedCard ?? ''}:${ability.declaredLevel ?? ''}`
  }
  return `profession:${ability.id}:${ability.cards.join('-')}:${ability.targetCard ?? ''}:${ability.declaredElement ?? ''}:${ability.declaredLevel ?? ''}:${ability.inputRequirement?.type ?? ''}`
}

function knownCardLabel(cardId: number): string {
  const card = state.value.hands
    .flatMap(hand => hand.cards.kind === 'known' ? hand.cards.cards : [])
    .find(candidate => candidate.id === cardId)
  return card?.label ?? `牌 ${cardId}`
}

function playableActionDetail(action: PlayableAction): string {
  return presentPlayableAction(action)
}

function playableActionIdentity(action: PlayableAction): string {
  const parts: string[] = []
  if (action.type === 'activateProfessionAbility') {
    if (action.cards.length) parts.push(`所選牌：${action.cards.map(knownCardLabel).join('、')}`)
    if (action.targetCard !== null) parts.push(`指定牌：${knownCardLabel(action.targetCard)}`)
    if (action.declaredElement !== null) parts.push(`宣告屬性：${cardElementLabel(action.declaredElement)}`)
    if (action.declaredLevel !== null) parts.push(`宣告等級：${action.declaredLevel}`)
  } else if (action.type === 'useSpiritSkill') {
    if (action.selectedCard !== null) parts.push(`指定牌：${knownCardLabel(action.selectedCard)}`)
    if (action.declaredLevel !== null) parts.push(`宣告等級：${action.declaredLevel}`)
  }
  return parts.length ? `${playableActionName(action)}；${parts.join('；')}` : playableActionName(action)
}

function labelWithDetail(label: string, detail: string): string {
  return detail ? `${label}；${detail}` : label
}

function playableActionAccessibleLabel(action: PlayableAction): string {
  return labelWithDetail(playableActionIdentity(action), playableActionDetail(action))
}

function showActionDetail(action: PlayableAction) {
  clearTimeout(actionDetailTimer)
  actionDetail.value = playableActionDetail(action) || null
}

function showTextActionDetail(text: string) {
  clearTimeout(actionDetailTimer)
  actionDetail.value = text || null
}

function hideActionDetail() {
  clearTimeout(actionDetailTimer)
  actionDetail.value = null
}

function startActionDetail(action: PlayableAction) {
  clearTimeout(actionDetailTimer)
  actionDetailTimer = setTimeout(() => {
    actionDetail.value = playableActionDetail(action) || null
  }, 450)
}

function cancelActionDetail() {
  clearTimeout(actionDetailTimer)
}

function phaseLabel(value: string): string {
  const labels: Record<string, string> = {
    ActiveEffects: '效果處理',
    Action: '行動',
    MainPhase: '主要階段',
    TurnStart: '回合開始',
    TurnDraw: '回合抽牌',
    TurnEnd: '回合結束',
  }
  return labels[value] ?? value
}

function environmentLabel(value: PublicGameState['environment']): string {
  if (!value) return '無環境'
  return {
    Metal: '金行',
    Wood: '木行',
    Water: '水行',
    Fire: '火行',
    Earth: '土行',
  }[value]
}

function pendingChoiceLabel(choice: PublicGameState['pendingChoice']): string {
  if (!choice) return '等待選擇'
  return presentPendingChoice(choice.reason)
}

function formationChoiceLabel(formationId: string): string {
  const labels: Record<string, string> = {
    'echo:ringing-metal': '商調‧鳴金',
    'echo:falling-wood': '角調‧落木',
    'echo:flowing-water': '羽調‧流水',
    'echo:war-fire': '徵調‧戰火',
    'echo:split-earth': '宮調‧裂土',
    'echo:pure-fire': '變徵‧淨火',
    'echo:plant-earth': '變宮‧植土',
  }
  return labels[formationId] ?? formationId
}

</script>

<style>
@reference "../../assets/css/main.css";

@scope (.game-page) {
:scope { @apply flex h-[calc(100vh-84px)] flex-col overflow-hidden max-[900px]:h-auto max-[900px]:overflow-visible; }
.back-button { @apply grid size-9 place-items-center border border-[var(--app-border-strong)] bg-[rgba(17,23,19,.88)] text-base text-[var(--app-text)] hover:border-[var(--app-accent)] hover:text-gold-light; }
.battlefield-back { @apply absolute top-4 left-4 z-20; }
.battle-layout { @apply grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_330px] max-[900px]:grid-cols-1 max-[900px]:overflow-auto; }
.battlefield {
  --card-back-base: #232e28;
  --card-back-stripe: #344039;
  --card-back-border: #85714a;
  position: relative;
  min-width: 0;
  display: grid;
  grid-template-areas:
    "top top top"
    "left center right"
    "bottom bottom bottom";
  grid-template-columns: minmax(108px, .6fr) minmax(300px, 1.8fr) minmax(108px, .6fr);
  grid-template-rows: minmax(150px, .8fr) minmax(230px, 1.15fr) minmax(175px, 1fr);
  gap: 8px 14px;
  padding: 22px 36px;
  overflow: hidden;
  background: radial-gradient(ellipse at center, var(--app-battlefield-center) 0%, var(--app-battlefield-mid) 58%, var(--app-battlefield-edge) 100%);
}
.battlefield::before { content: ""; position: absolute; inset: 22px; border: 1px solid rgba(175, 143, 79, .18); pointer-events: none; }
.battlefield::after { content: "五 行"; position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 270px; height: 270px; border: 1px solid rgba(183, 148, 77, .1); border-radius: 50%; display: grid; place-items: center; color: rgba(204, 171, 100, .06); font-family: serif; font-size: 70px; pointer-events: none; }
.player-seat { @apply relative z-1 flex min-h-0 min-w-0 items-center justify-center gap-4; }
.seat-top { grid-area: top; }
.seat-bottom { grid-area: bottom; flex-direction: row-reverse; }
.seat-left { grid-area: left; flex-direction: column; }
.seat-right { grid-area: right; flex-direction: column; }
.player-identity { @apply flex min-w-0 flex-wrap items-center gap-2.5; }
.player-identity div { @apply grid; }
.player-identity strong { @apply max-w-36 truncate text-xs; }
.player-identity small { @apply text-[11px] text-[#d0a450]; }
.profession-badge { @apply relative cursor-help border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-1.5 py-0.5 text-[10px] text-gold-light outline-none focus-visible:border-[#d1ad62]; }
.profession-summary { @apply invisible absolute top-[calc(100%+6px)] left-0 z-20 grid w-64 gap-1 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-2.5 text-left opacity-0 shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.profession-summary b { @apply font-serif text-xs text-gold-light; }
.profession-summary small { @apply whitespace-normal text-[10px]! leading-4 text-muted!; }
.profession-badge:hover .profession-summary, .profession-badge:focus .profession-summary, .profession-badge:focus-within .profession-summary { @apply visible opacity-100; }
.connection-dot { @apply size-2 shrink-0 rounded-full border border-[#76524b] bg-[#6f3c34]; }
.connection-dot.connected { @apply border-[#477557] bg-[#63a979]; }
.reconnecting-label { @apply text-[9px] text-[#d0aa5e]; }
.turn-badge { @apply border border-[#477557] bg-[#16251b] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#77bd8d]!; }
.counter-badge { @apply border border-[#8a733b] bg-[#292415] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#d5b868]!; }
.shield-badge { @apply border border-[#557684] bg-[#17262c] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#8fc1d5]!; }
.status-badge { @apply border border-[#765557] bg-[#28191b] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#d49a9a]!; }
.side-hand-count { @apply hidden text-[9px] text-muted; }
.hand { @apply flex min-w-0 items-center justify-center gap-2; }
.seat-top .playing-card { width: clamp(48px, 5vw, 68px); }
.seat-top .card-element { @apply size-7 text-sm; }
.seat-left .seat-hand, .seat-right .seat-hand { @apply flex-col gap-1; }
.seat-left .playing-card, .seat-right .playing-card { width: 30px; }
.seat-left .card-level, .seat-right .card-level { @apply top-0.5 text-[8px]; }
.seat-left .card-element, .seat-right .card-element { @apply size-4 text-[10px]; }
.board-center { grid-area: center; @apply relative z-1 grid min-w-0 grid-cols-[90px_minmax(220px,1fr)_90px] items-center justify-items-center; }
.battlefield.discard-open { z-index: 25; overflow: visible; }
.battlefield.discard-open .board-center { z-index: 16; }
.discard-piles { @apply z-2 flex w-full justify-end; grid-column: 1 / -1; grid-row: 1; }
.discard-piles.personal { @apply pointer-events-none absolute inset-0 block; }
.discard-composition-layer { @apply absolute right-0 z-20; bottom: calc(50% + 48px); }
.discard-composition {
  @apply w-[300px] border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3.5 text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)];
}
.discard-composition h2 { @apply mb-2.5 font-serif text-sm text-gold-light; }
.card-composition table { @apply w-full table-fixed border-collapse; }
.card-composition th, .card-composition td { @apply h-8 border border-[var(--app-border)] text-center; }
.card-composition thead th { @apply text-[10px] font-bold text-[var(--app-text)]; }
.card-composition tbody th { @apply w-7 text-[10px] font-normal text-muted; }
.card-composition td strong { @apply font-serif text-sm text-[#e4c47d]; }
.card-composition td.empty strong { color: var(--app-text-soft); }
.card-composition tbody tr:nth-child(1) > th { color: #ded5ba; }
.card-composition tbody tr:nth-child(2) > th { color: #77a980; }
.card-composition tbody tr:nth-child(3) > th { color: #75a8bd; }
.card-composition tbody tr:nth-child(4) > th { color: #d17a6c; }
.card-composition tbody tr:nth-child(5) > th { color: #c8a265; }
.pouch-composition { @apply mx-auto mt-5 w-[min(390px,calc(100vw-64px))] border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3.5 text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.pouch-composition td { @apply p-0; }
.pouch-composition td button { @apply grid size-full min-h-8 place-items-center border-0 bg-transparent text-[#e4c47d] hover:bg-[rgba(185,149,80,.16)] disabled:cursor-not-allowed disabled:opacity-45; }
.pouch-composition td button[aria-pressed="true"] { @apply bg-[rgba(185,149,80,.3)] shadow-[inset_0_0_0_2px_#d1ad62]; }
.choice-card-matrix td button small { @apply text-[8px] font-normal text-[#f0d99e]; }
.chain-composition { @apply mt-2; }
.choice-selection-summary { @apply mx-auto mb-1 flex max-w-[390px] items-center justify-between gap-3 text-xs text-gold-light; }
.choice-selection-summary button { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-2 py-1 text-[10px] text-[var(--app-text)] hover:border-[var(--app-accent)]; }
.formation-field { @apply relative z-3 grid min-h-48 w-full min-w-0 grid-rows-[auto_1fr_auto] items-center border-x border-[rgba(166,141,86,.14)] px-3 py-2 text-center text-[10px] text-[var(--app-text-muted)]; grid-column: 2; grid-row: 1; }
.formation-field-heading { @apply flex flex-wrap items-center justify-center gap-2; }
.formation-field-label { @apply text-[#9a8251]; letter-spacing: .2em; }
.environment-badge { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-2 py-1 text-[9px] text-gold-light; }
.previous-formation { @apply grid min-h-24 content-center justify-items-center gap-1.5; }
.previous-formation small { @apply text-[9px] text-muted; }
.previous-formation strong { @apply font-serif text-sm text-gold-light; }
.previous-formation p { @apply text-[10px] text-[var(--app-text-muted)]; }
.formation-cards { @apply flex min-h-12 items-center justify-center; }
.formation-card { width: 34px; margin-left: -4px; }
.turn-controls { @apply relative grid min-h-14 content-center gap-2 border-t border-[rgba(166,141,86,.14)] pt-2; }
.ability-panel, .action-panel { @apply grid gap-1.5 border p-2; border-color: color-mix(in srgb, var(--app-accent) 24%, transparent); border-radius: 9px; background: color-mix(in srgb, var(--app-surface-muted) 84%, transparent); }
.ability-panel header, .action-panel header { @apply flex flex-wrap items-baseline justify-between gap-x-2 text-left; }
.ability-panel h3, .action-panel h3 { @apply font-serif text-xs text-gold-light; }
.ability-panel small, .action-panel small { @apply text-[9px] text-muted; }
.action-candidates { @apply flex max-w-full flex-wrap justify-center gap-1.5; }
.action-candidates button { @apply min-h-8 border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2.5 py-1.5 text-[10px] text-[var(--app-text)] hover:border-[var(--app-accent)]; }
.action-candidates p { @apply text-[9px] text-[var(--app-text-muted)]; }
.spirit-level-picker { @apply relative; }
.spirit-level-trigger { @apply grid min-h-8 min-w-12 place-items-center border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2.5 py-1.5 text-[10px] text-[var(--app-text)] hover:border-[var(--app-accent)]; }
.spirit-level-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 2px; }
.spirit-level-options { @apply absolute bottom-[calc(100%+5px)] left-1/2 z-10 grid min-w-20 -translate-x-1/2 gap-1 border border-[var(--app-accent)] bg-[var(--app-surface-muted)] p-1 shadow-[0_10px_24px_rgba(0,0,0,.45)]; }
.action-candidates .spirit-level-options button { @apply min-h-7 whitespace-nowrap px-2 py-1; }
.action-candidates .skip-action { @apply border-[var(--app-accent)] text-gold-light; }
.action-processing { @apply text-[#d0aa5e]; }
.action-error { @apply text-[#d79587]; }
.action-prompt { @apply text-[var(--app-text-muted)]; }
.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.action-detail strong { @apply mr-2 text-gold-light; }
.choice-overlay,
.choice-waiting-overlay { @apply absolute inset-0 z-12 grid place-items-center bg-[var(--app-choice-overlay)] text-center; }
.choice-overlay > div,
.choice-waiting-overlay > div { @apply max-h-[calc(100%-32px)] min-w-90 overflow-y-auto border border-[var(--app-accent)] p-[30px]; border-radius: 16px; background: var(--app-surface-raised); box-shadow: var(--app-shadow-lg); }
.choice-overlay h2,
.choice-waiting-overlay h2 { @apply mt-2.5 mb-5 font-serif; }
.virtual-formation-card-dialog { @apply max-w-[min(560px,calc(100vw-32px))]; }
.virtual-formation-card-matrix { @apply mx-auto border-collapse text-xs; }
.virtual-formation-card-matrix th { @apply border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2 py-1.5 font-normal text-muted; }
.virtual-formation-card-matrix tbody th { @apply min-w-14 text-gold-light; }
.virtual-formation-card-matrix td { @apply border border-[var(--app-border-strong)] p-0; }
.virtual-formation-card-option { @apply grid size-11 place-items-center bg-[var(--app-surface-subtle)] font-serif text-sm text-[#e5dfd1] hover:bg-[#3a443d] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.virtual-formation-card-option:focus-visible { @apply relative z-1 outline-2 outline-offset-[-3px] outline-[#d1ad62]; }
.virtual-formation-card-cancel { @apply mt-5 min-h-9 border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-4 py-2 text-xs text-[var(--app-text)] hover:border-[var(--app-accent)] hover:text-gold-light; }
.choice-cards { @apply flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-cards .choice-card { width: clamp(76px, 11vw, 112px); }
.choice-options { @apply mt-3 flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-options button { @apply min-h-10 border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-3 py-2 text-xs text-[var(--app-text)] hover:border-[var(--app-accent)] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.choice-options button:focus-visible { @apply border-[#d1ad62] outline-2 outline-offset-2 outline-[#d1ad62]; }
.choice-options button.selected,
.choice-options button[aria-pressed="true"] { @apply font-bold; border-color: var(--app-accent); background: var(--app-accent); color: var(--app-on-accent); box-shadow: 0 0 0 2px var(--app-accent-soft); }
.choice-actions { @apply mt-5; }
.choice-actions .choice-confirm { @apply border-[var(--app-accent)] bg-[var(--app-accent)] font-bold hover:bg-[var(--app-accent-strong)]; color: var(--app-on-accent); }
.choice-count { @apply mt-4 text-xs text-muted; }
.choice-submit { @apply mt-3 border border-[var(--app-accent)] bg-[var(--app-accent)] px-5 py-2 text-xs font-bold disabled:cursor-not-allowed disabled:opacity-45; color: var(--app-on-accent); }
.setup-reveal { @apply absolute inset-0 z-15 grid place-items-center bg-[var(--app-overlay)] text-center backdrop-blur-[5px]; }
.setup-reveal > div { @apply grid w-[min(520px,calc(100vw-32px))] gap-4 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-7; }
.setup-reveal h2 { @apply font-serif text-2xl text-gold-light; }
.setup-reveal ol { @apply m-0 grid list-none grid-cols-4 gap-2 p-0 max-[600px]:grid-cols-2; counter-reset: order; }
.setup-reveal li { @apply border border-[var(--app-border)] bg-[var(--app-surface-muted)] p-2 text-xs; counter-increment: order; }
.setup-reveal li::before { content: counter(order) ". "; color: #c6a35e; }
.revealed-teams { @apply grid grid-cols-2 gap-3; }
.revealed-teams span { @apply grid gap-1 border border-[var(--app-border)] p-3 text-xs text-muted; }
.revealed-teams strong { @apply text-gold-light; }
.waiting-members { @apply grid grid-cols-2 gap-3; }
.waiting-members span { @apply grid gap-1 border border-[var(--app-border)] bg-[var(--app-surface-muted)] p-3 text-sm text-muted; }
.waiting-members span.joined { @apply border-[var(--app-accent)] text-[var(--app-text)]; }
.waiting-members small { @apply text-[10px] text-muted; }
.waiting-members button { @apply mt-1 border-0 bg-transparent text-[9px] text-[#c98e82]; }
.invite-link { @apply justify-self-start border-0 bg-transparent text-xs text-gold-light; }
.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }
.result-actions .ghost-button { @apply border-[var(--app-border-strong)] text-[var(--app-text)]; }
.result-actions .primary-button { @apply justify-between; }

.game-sidebar { @apply grid min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden border-l border-line bg-panel max-[900px]:border-l-0; }
.game-sidebar.finished { grid-template-rows: auto minmax(0, 1fr); }
.result-panel { @apply border-b border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-5; }
.result-panel h2 { @apply font-serif text-2xl text-gold-light; }
.result-panel p { @apply mt-1 text-xs text-muted; }
.result-panel .result-actions { @apply grid-cols-1; }
.panel-title { @apply flex items-start justify-between; }
.event-panel { @apply min-h-0 overflow-auto border-b border-line p-5; }
.panel-title h2 { @apply font-serif text-[15px]; }
.event-panel .event-expand-button { @apply hidden border-0 bg-transparent text-[10px] text-gold-light; }
.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }
.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }
.event-feed li > i { width: 5px; height: 5px; border-radius: 50%; background: #b79550; margin-top: 6px; box-shadow: 0 0 0 4px rgba(183, 149, 80, .08); }
.event-feed span { color: var(--app-text); font-size: 10px; font-weight: 700; }
.event-feed p { color: var(--app-text-muted); font-size: 9px; line-height: 1.45; margin-top: 2px; }
@media (min-width: 901px) {
  .player-identity strong { font-size: 14px; }
  .player-identity small { font-size: 12px; }
  .reconnecting-label, .turn-badge, .counter-badge, .shield-badge, .status-badge, .side-hand-count { font-size: 11px!important; }
  .formation-field { font-size: 12px; }
  .previous-formation small { font-size: 11px; }
  .previous-formation p, .action-candidates button { font-size: 12px; }
  .panel-title h2 { font-size: 17px; }
  .event-feed span { font-size: 12px; }
  .event-feed p { font-size: 11px; }
}

@media (max-width: 900px) {
  .battle-layout { grid-template-columns: 1fr; overflow: auto; }
  .game-page { height: auto; overflow: visible; }
  .battlefield { min-height: 720px; padding: 22px; }
  .discard-composition-layer {
    @apply fixed inset-0 grid place-items-center bg-[var(--app-overlay)] p-4 backdrop-blur-[3px];
  }
  .discard-composition { width: min(330px, calc(100vw - 32px)); }
  .game-sidebar { border-left: 0; }
  .event-panel .event-expand-button { display: block; }
  .event-panel:not(.expanded) .event-feed li:nth-child(n+4) { display: none; }
}

@media (max-width: 600px) {
  .battlefield {
    min-height: 690px;
    grid-template-columns: 76px minmax(0, 1fr) 76px;
    grid-template-rows: 165px minmax(250px, 1fr) 185px;
    gap: 4px;
    padding: 46px 8px 12px;
  }
  .battlefield::before { inset: 8px; }
  .battlefield-back { top: 10px; left: 10px; }
  .player-seat { gap: 7px; }
  .seat-top, .seat-bottom { flex-direction: column; }
  .seat-top .player-identity, .seat-bottom .player-identity { order: 2; }
  .seat-left .seat-hand, .seat-right .seat-hand { display: none; }
  .seat-left .player-identity, .seat-right .player-identity { @apply flex-col gap-1 text-center; }
  .seat-left .player-identity strong, .seat-right .player-identity strong { @apply max-w-18 text-[9px]; }
  .seat-left .player-identity small, .seat-right .player-identity small { @apply text-[9px]; }
  .seat-left .turn-badge, .seat-right .turn-badge, .seat-left .counter-badge, .seat-right .counter-badge, .seat-left .shield-badge, .seat-right .shield-badge, .seat-left .status-badge, .seat-right .status-badge { @apply max-w-18 whitespace-normal px-1 py-0.5 text-[8px]!; }
  .seat-left .side-hand-count, .seat-right .side-hand-count { @apply block; }
  .board-center { grid-template-columns: 48px minmax(0, 1fr) 48px; width: 100%; }
  .formation-field { min-width: 0; width: 100%; }
  .playing-card { width: 54px; }
  .seat-top .playing-card { width: 43px; }
  .player-identity strong { max-width: 110px; }
  .turn-badge { font-size: 8px!important; }
  .action-candidates button { min-height: 30px; padding: 4px 7px; }
  .waiting-members { grid-template-columns: 1fr 1fr; }
}
}
</style>
