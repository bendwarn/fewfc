<template>
  <main v-if="!routeReady" class="route-state" aria-live="polite">
    <div class="route-state-content"><span class="route-spinner" aria-hidden="true" /><h1>正在載入</h1><p>正在恢復房間狀態。</p></div>
  </main>
  <main v-else-if="roomRouteError" class="route-state">
    <div class="route-state-content"><h1>{{ roomRouteError }}</h1><button class="primary-button" type="button" @click="returnToLobby">返回房間大廳</button></div>
  </main>
  <main v-else class="game-page">
    <div class="battle-layout">
        <BattlefieldBoard
          ref="battlefield"
          :state="state"
          :display-names="displayNames"
          :anchor-player="ownPlayer || null"
          mode="live"
          :selected-card-ids="game.selectedCards.value"
          :selectable-player="game.canSelectCard(ownPlayer) ? ownPlayer : null"
          :cards-disabled="!roomConnected"
          :connected-players="connectedPlayers"
          :reconnecting-player="game.connectionState.value === 'reconnecting' ? ownPlayer : null"
          :show-turn-controls="!roomWaiting && !gameFinished && viewer === state.currentPlayer"
          @select-card="game.toggleCardSelection"
        >
          <template #before>
            <button class="back-button battlefield-back" type="button" aria-label="返回房間列表" title="返回房間列表" @click="leaveGame">←</button>
          </template>
          <template #board-overlay>
            <GameConclusionPanel
              v-if="gameFinished"
              class="battlefield-conclusion"
              :state="state"
              :team-label="teamLabel"
              :summary="`${firstTurnText}，本局已結束。`"
            >
              <template #actions>
                <button class="primary-button" type="button" :disabled="game.isLoading.value" @click="restartGame">
                  返回房間 <span>→</span>
                </button>
              </template>
            </GameConclusionPanel>
          </template>
          <template #turn-controls>
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
          </template>
          <template #overlay>

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
            :aria-label="`秘計‧${strategyLabel(secretStrategyOptionStrategy(secretStrategyDraft))}：選擇輸入`"
          >
            <div>
              <h2>秘計‧{{ strategyLabel(secretStrategyOptionStrategy(secretStrategyDraft)) }}</h2>
              <p class="action-detail">{{ presentSecretStrategyOption(secretStrategyDraft) }}</p>

              <div
                v-if="secretStrategyDraft.type === 'targetPlayer'"
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

              <template v-if="secretStrategyDraft.type === 'star'">
                <h3>瞞天：取得星辰效果或破除星辰</h3>
                <div class="choice-options" aria-label="瞞天選擇">
                  <button
                    v-for="star in secretStrategyDraft.gainStars"
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

              <template v-if="secretStrategyDraft.type === 'environment'">
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
                    v-for="card in ownHandCards"
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
                先選擇錦囊給予對象，再依五行與等級選一張作為錦囊；也可再選一張公開觸發秘計。
              </p>
              <p v-else>各選 {{ pouchSwapRequiredCount }} 張牌組牌與棄牌交換，之後洗牌。</p>

              <template v-if="pouchChoiceKind === 'chain'">
                <ChainChoice
                  :pouch-owners="chainPouchOwners"
                  :pouch-owner="effectiveChainPouchOwner"
                  :pouch-cards="chainPouchCards"
                  :pouch-card="chainPouchCard"
                  :trigger-cards="chainTriggerCards"
                  :trigger-card="chainTriggerCard"
                  :strategy-options="chainStrategyOptions"
                  :selected-strategy="chainStrategySelection"
                  :selected-strategy-action="selectedChainStrategyAction"
                  :selected-target="strategyTargetSelection"
                  :selected-star="strategyStarSelection"
                  :break-star="strategyBreakStar"
                  :selected-retreat-card="strategyDiscardCard"
                  :hand-cards="ownHandCards"
                  :player-label="playerLabel"
                  :strategy-label="strategyLabel"
                  :star-label="starLabel"
                  @select-pouch-owner="pouchOwnerSelection = $event"
                  @choose-pouch-card="chooseChainPouchCard"
                  @clear-pouch-card="clearChainPouchCard"
                  @choose-trigger-card="chooseChainTriggerCard"
                  @clear-trigger-card="clearChainTriggerCard"
                  @select-strategy="chainStrategySelection = $event"
                  @select-target="strategyTargetSelection = $event"
                  @select-star="selectChainStar"
                  @select-retreat-card="strategyDiscardCard = $event"
                />
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

          </template>
        </BattlefieldBoard>

        <button
          v-if="!roomWaiting && latestVisibleEvent"
          ref="eventSheetTrigger"
          class="mobile-event-summary"
          type="button"
          aria-haspopup="dialog"
          :aria-expanded="eventSheetOpen"
          aria-controls="mobile-event-sheet"
          @click="openEventSheet"
        >
          <strong>戰局紀錄</strong>
          <span>{{ latestVisibleEvent.title }} · {{ latestVisibleEvent.summary }}</span>
        </button>

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

        <aside class="game-sidebar">
          <section ref="desktopEventFeed" class="event-panel">
            <div class="panel-title">
              <h2>戰局紀錄 <button v-if="roomWaiting && game.savableReplay.value" class="ghost-button" type="button" :disabled="replaySaving" @click="saveCurrentReplay">{{ replaySaved ? '已儲存' : '儲存本局' }}</button></h2>
            </div>
            <p v-if="replayError" class="form-error" role="alert">
              {{ replayError }}
              <button class="ghost-button" type="button" :disabled="replaySaving" @click="saveCurrentReplay">重試</button>
            </p>
            <ol class="event-feed">
              <template v-for="group in battleRecordGroups" :key="group.id">
                <li class="event-group-title"><strong>{{ group.title }}</strong></li>
                <li v-for="entry in group.entries" :key="entry.id"><i /><div><span>{{ entry.title }}</span><p v-if="entry.summary">{{ entry.summary }}</p></div></li>
              </template>
            </ol>
          </section>
        </aside>
      </div>

      <Teleport to="body">
        <div v-if="eventSheetOpen" class="event-sheet-portal">
          <button class="event-sheet-backdrop" type="button" tabindex="-1" aria-label="關閉戰局紀錄" @click="closeEventSheet" />
          <section
            id="mobile-event-sheet"
            class="event-sheet"
            role="dialog"
            aria-modal="true"
            aria-labelledby="mobile-event-sheet-title"
            @keydown.stop="handleEventSheetKeydown"
          >
            <header>
              <h2 id="mobile-event-sheet-title">戰局紀錄</h2>
              <button ref="eventSheetClose" type="button" aria-label="關閉戰局紀錄" @click="closeEventSheet">×</button>
            </header>
            <button v-if="unseenEventCount" class="new-event-button" type="button" @click="scrollEventSheetToBottom">
              {{ unseenEventCount }} 筆新紀錄
            </button>
            <ol ref="eventSheetFeed" class="event-feed event-sheet-feed" @scroll.passive="updateEventSheetFollow">
              <template v-for="group in battleRecordGroups" :key="`mobile-${group.id}`">
                <li class="event-group-title"><strong>{{ group.title }}</strong></li>
                <li v-for="entry in group.entries" :key="`mobile-${entry.id}`"><i /><div><span>{{ entry.title }}</span><p v-if="entry.summary">{{ entry.summary }}</p></div></li>
              </template>
            </ol>
          </section>
        </div>
      </Teleport>
  </main>
</template>

<script setup lang="ts">
import type {
  CardInstanceId,
  Element,
  PlayableAction,
  PlayerId,
  PublicCard,
  PublicGameState,
  SecretStrategy,
  SecretStrategyOption,
  StarKind,
  TeamId,
  ViewerId,
} from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '#shared/game-room'
import { createRuleModulePolicy, presentationForRuleModule } from '#shared/utils/rule-modules'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import { presentDirectSecretStrategyAction, presentDiscardRetrievalAction, presentPlayableAction, presentSecretStrategyOption } from '~/lib/action-detail-presentation'
import { splitEarthChoiceKey, usesSplitEarthFormationGroups } from '#shared/utils/split-earth-formation-choice'
import { roomRouteResult } from '~/lib/navigation'
import { cardChoiceDraftCount, chainChoiceAnswer, isImmediateCardChoice, toggleChoiceCard } from '~/lib/pending-choice-interaction'
import { secretStrategyDraftAction, secretStrategyOptionStrategy } from '~/lib/secret-strategy-draft'
import { sheepReturnCards } from '~/lib/pouch-choice'
import { useLayoutNotifications } from '~/lib/player-notifications-context'
import { useRulesCatalog } from '~/lib/rules-catalog'
import { presentApiError } from '~/lib/api-error-presentation'
import { abilityLevelPicker } from '~/lib/ability-level-picker'
import { battleRecordContentRevision, scrollBattleRecordToLatest } from '~/lib/battle-record-scroll'
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
const battlefield = ref<{
  root: HTMLElement | null
  hasOpenDetail: () => boolean
  closeDetail: () => void
} | null>(null)

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
  const requirements = game.playableSecretStrategies.value
    .filter(requirement => requirement.sourceCard === pouch.id)
  return requirements.map((requirement) => {
    const strategy = secretStrategyOptionStrategy(requirement)
    return {
      label: `秘計‧${strategyLabel(strategy)}`,
      detail: presentDirectSecretStrategyAction(requirement),
      strategy,
      requirement,
    }
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
const effectiveChainPouchOwner = computed(() => (
  pouchOwnerSelection.value
  ?? (chainPouchOwners.value.length === 1 ? chainPouchOwners.value[0] ?? null : null)
))
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
const chainTriggerCards = computed(() => chainPouchCards.value)
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
  return choice.choice.strategyOptions
    .filter(option => option.sourceCard === card.id)
})
const selectedChainStrategyAction = computed(() => chainStrategyOptions.value.find(
  option => secretStrategyOptionStrategy(option) === chainStrategySelection.value,
) ?? null)
const chainStrategyDecision = computed(() => {
  const option = selectedChainStrategyAction.value
  return option
    ? secretStrategyDraftAction(option, {
        targetPlayer: strategyTargetSelection.value,
        star: strategyStarSelection.value,
        breakStar: strategyBreakStar.value,
        retreat: strategyDiscardCard.value ?? 'clearEnvironment',
      })
    : undefined
})
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
    || !effectiveChainPouchOwner.value
    || pouchDeckSelection.value.length < 1
    || pouchDeckSelection.value.length > 2) return false
  if (pouchDeckSelection.value.length === 1) return true
  return chainStrategyDecision.value !== undefined
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

function selectChainStar(star: StarKind, breakStar: boolean) {
  strategyStarSelection.value = star
  strategyBreakStar.value = breakStar
}

function startPouchAction(action: PouchStrategyAction) {
  if (action.requirement.type === 'noInput' || action.requirement.type === 'sheepStealing') {
    const decision = secretStrategyDraftAction(action.requirement, {})
    if (decision) void game.triggerSecretStrategy(decision)
    return
  }

  resetSecretStrategyDraft()
  secretStrategyDraft.value = action.requirement
}

async function submitSecretStrategyDraft() {
  const action = secretStrategyAction.value
  if (!action) return

  if (await game.triggerSecretStrategy(action)) {
    resetSecretStrategyDraft()
  }
}

function startPlayableAction(action: PlayableAction) {
  if (action.type === 'performFormation' && action.id === 'pouch:chain') {
    resetPouchChoice()
    void game.performPlayableAction(action).then((submitted) => {
      if (submitted) {
        pouchChoiceKind.value = 'chain'
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
  const owner = effectiveChainPouchOwner.value
  const pouchCard = pouchDeckSelection.value[0]
  if (!owner || pouchCard === undefined) return
  const trigger = pouchDeckSelection.value[1]
  const decision = trigger ? chainStrategyDecision.value : undefined
  if (trigger && !decision) return
  const submitted = await game.answerChainChoice(chainChoiceAnswer({
    decision: decision
      ? { type: 'placeAndTrigger', pouchOwner: owner, pouchCard, decision }
      : { type: 'placeOnly', pouchOwner: owner, pouchCard },
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
const showSetupReveal = ref(false)
const eventSheetOpen = ref(false)
const eventSheetTrigger = ref<HTMLButtonElement | null>(null)
const eventSheetClose = ref<HTMLButtonElement | null>(null)
const eventSheetFeed = ref<HTMLOListElement | null>(null)
const desktopEventFeed = ref<HTMLElement | null>(null)
const eventSheetFollowing = ref(true)
const unseenEventCount = ref(0)

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
const displayNames = computed<Record<string, string>>(() => Object.fromEntries(
  (onlineMetadata.value?.members ?? []).map(member => [member.player, member.displayName]),
))
const connectedPlayers = computed(() => (onlineMetadata.value?.members ?? [])
  .filter(member => member.connected)
  .map(member => member.player))
const battleRecordGroups = computed(() => {
  const record = game.battleRecord.value
  return [
    { id: 'preparation', title: '對局準備', entries: record.preparation.entries },
    ...record.turns.map(group => ({ id: `turn-${group.turnNumber}`, title: group.title, entries: group.entries })),
  ].filter(group => group.id !== 'preparation' || group.entries.length > 0)
})
const latestVisibleEvent = computed(() => battleRecordGroups.value.flatMap(group => group.entries).at(-1) ?? null)
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
const activeTeams = computed(() => [...new Set(state.value.players.map((player) => player.team))])
const showSkip = computed(() => (
  roomConnected.value
  && game.playablePass.value !== null
))
let actionDetailTimer: ReturnType<typeof setTimeout> | undefined
let setupRevealTimer: ReturnType<typeof setTimeout> | undefined

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
  showSetupReveal.value = false
  eventSheetOpen.value = false
  eventSheetFollowing.value = true
  unseenEventCount.value = 0
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
  battlefield.value?.closeDetail()
  closeSplendorMenu()
  closeDarkSpiritMenu()
}

function openEventSheet() {
  eventSheetOpen.value = true
  eventSheetFollowing.value = true
  unseenEventCount.value = 0
  void nextTick(() => {
    scrollEventSheetToBottom()
    eventSheetClose.value?.focus()
  })
}

function closeEventSheet() {
  if (!eventSheetOpen.value) return
  eventSheetOpen.value = false
  void nextTick(() => eventSheetTrigger.value?.focus())
}

function scrollEventSheetToBottom() {
  const feed = eventSheetFeed.value
  if (feed) feed.scrollTop = feed.scrollHeight
  eventSheetFollowing.value = true
  unseenEventCount.value = 0
}

function updateEventSheetFollow() {
  const feed = eventSheetFeed.value
  if (!feed) return
  eventSheetFollowing.value = feed.scrollHeight - feed.scrollTop - feed.clientHeight < 20
  if (eventSheetFollowing.value) unseenEventCount.value = 0
}

function handleEventSheetKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    closeEventSheet()
    return
  }
  if (event.key !== 'Tab') return
  const focusable = Array.from(document.querySelectorAll<HTMLElement>(
    '#mobile-event-sheet button:not([disabled]), #mobile-event-sheet [href], #mobile-event-sheet [tabindex]:not([tabindex="-1"])',
  ))
  if (!focusable.length) return
  const first = focusable[0]!
  const last = focusable.at(-1)!
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
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
    battlefield.value?.hasOpenDetail()
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
  const buttons = battlefield.value?.root?.querySelectorAll<HTMLButtonElement>(
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

  if (eventSheetOpen.value) {
    event.preventDefault()
    closeEventSheet()
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

  if (battlefield.value?.hasOpenDetail()) {
    event.preventDefault()
    battlefield.value.closeDetail()
    return
  }

}



async function restartGame() {
  await game.resetOnlineRoom()
  // 返回房間開始新遊戲時重設回放儲存狀態。
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
  () => onlineMetadata.value?.status,
  (status, previous) => {
    if (status !== 'Active' || previous === 'Active') return
    clearTimeout(setupRevealTimer)
    showSetupReveal.value = true
    setupRevealTimer = setTimeout(() => { showSetupReveal.value = false }, 1800)
  },
)
watch(() => game.roomDissolved.value, dissolved => { if (dissolved) void router.replace('/rooms') })
watch(() => battleRecordContentRevision(battleRecordGroups.value), (revision, previous = revision) => {
  if (revision === previous) return
  void nextTick(() => {
    scrollBattleRecordToLatest(desktopEventFeed.value)
    if (eventSheetOpen.value) scrollEventSheetToBottom()
  })
})

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
    return `秘計‧${strategyLabel(secretStrategyOptionStrategy(action.option))}`
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
:scope { @apply flex min-h-0 flex-1 flex-col overflow-hidden; }
.back-button { @apply grid size-9 place-items-center border border-[var(--app-border-strong)] bg-[rgba(17,23,19,.88)] text-base text-[var(--app-text)] hover:border-[var(--app-accent)] hover:text-gold-light; }
.battlefield-back { @apply absolute top-4 left-4 z-20; }
.battle-layout { @apply grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_330px]; }
.mobile-event-summary { @apply hidden; }
.pouch-composition { @apply mx-auto mt-5 w-[min(390px,calc(100vw-64px))] border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3.5 text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.pouch-composition td { @apply p-0; }
.pouch-composition td button { @apply grid size-full min-h-8 place-items-center border-0 bg-transparent text-[#e4c47d] hover:bg-[rgba(185,149,80,.16)] disabled:cursor-not-allowed disabled:opacity-45; }
.pouch-composition td button[aria-pressed="true"] { @apply bg-[rgba(185,149,80,.3)] shadow-[inset_0_0_0_2px_#d1ad62]; }
.choice-card-matrix td button small { @apply text-[8px] font-normal text-[#f0d99e]; }
.chain-composition { @apply mt-2; }
.choice-selection-summary { @apply mx-auto mb-1 flex max-w-[390px] items-center justify-between gap-3 text-xs text-gold-light; }
.choice-selection-summary button { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-2 py-1 text-[10px] text-[var(--app-text)] hover:border-[var(--app-accent)]; }
.turn-controls { @apply relative grid min-h-full min-w-0 content-start gap-2 border-l border-[rgba(166,141,86,.14)] pl-2; }
.ability-panel, .action-panel { @apply grid min-w-0 gap-1.5 border p-2; border-color: color-mix(in srgb, var(--app-accent) 24%, transparent); border-radius: 9px; background: color-mix(in srgb, var(--app-surface-muted) 84%, transparent); }
.ability-panel header, .action-panel header { @apply flex flex-wrap items-baseline justify-between gap-x-2 text-left; }
.ability-panel h3, .action-panel h3 { @apply font-serif text-xs text-gold-light; }
.ability-panel small, .action-panel small { @apply text-[9px] text-muted; }
.action-candidates { @apply flex min-w-0 max-w-full flex-wrap justify-center gap-1.5; }
.action-candidates button { @apply min-h-8 max-w-full border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2.5 py-1.5 text-[10px] text-[var(--app-text)] hover:border-[var(--app-accent)]; }
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
.action-detail { @apply pointer-events-none sticky bottom-0 z-8 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
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
.panel-title { @apply flex items-start justify-between; }
.event-panel { @apply min-h-0 overflow-auto border-b border-line p-5; }
.panel-title h2 { @apply font-serif text-[15px]; }
.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }
.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }
.event-feed li.event-group-title { grid-template-columns: minmax(0, 1fr); }
.event-feed li > i { width: 5px; height: 5px; border-radius: 50%; background: #b79550; margin-top: 6px; box-shadow: 0 0 0 4px rgba(183, 149, 80, .08); }
.event-feed span { color: var(--app-text); font-size: 10px; font-weight: 700; }
.event-feed p { color: var(--app-text-muted); font-size: 9px; line-height: 1.45; margin-top: 2px; }
@media (min-width: 901px) {
  .action-candidates button { font-size: 12px; }
  .panel-title h2 { font-size: 17px; }
  .event-feed span { font-size: 12px; }
  .event-feed p { font-size: 11px; }
}

@media (max-width: 900px) {
  .battle-layout { @apply grid-cols-1 grid-rows-[minmax(0,1fr)_auto] overflow-hidden; }
  .game-sidebar { @apply hidden; }
  .mobile-event-summary { @apply flex min-w-0 items-center gap-2 border-x-0 border-b-0 border-t border-line bg-panel px-3 py-2 text-left; }
  .mobile-event-summary strong { @apply shrink-0 font-serif text-[11px] text-gold-light; }
  .mobile-event-summary span { @apply min-w-0 flex-1 truncate text-[9px] text-muted; }
}

@media (max-width: 600px) {
  .battlefield-back { top: 10px; left: 10px; }
  .action-candidates button { min-height: 30px; padding: 4px 7px; }
  .waiting-members { grid-template-columns: 1fr 1fr; }
}
}

.event-sheet-portal { @apply fixed inset-0 z-50; }
.event-sheet-backdrop { @apply absolute inset-0 size-full cursor-default border-0 bg-[var(--app-overlay)] p-0 backdrop-blur-[3px]; }
.event-sheet { @apply fixed right-0 bottom-0 left-0 z-1 grid max-h-[72dvh] grid-rows-[auto_auto_minmax(0,1fr)] overflow-hidden rounded-t-2xl border-x-0 border-b-0 border-t border-[var(--app-accent)] bg-[var(--app-surface-raised)] text-[var(--app-text)] shadow-[0_-18px_48px_rgba(0,0,0,.45)]; }
.event-sheet > header { @apply flex items-center justify-between border-b border-line px-4 py-3; }
.event-sheet h2 { @apply font-serif text-base text-gold-light; }
.event-sheet header button { @apply grid size-8 place-items-center border border-[var(--app-border-strong)] bg-transparent text-xl text-muted; }
.event-sheet-feed { @apply m-0 grid min-h-0 list-none gap-3 overflow-y-auto p-4; overscroll-behavior: contain; }
.event-sheet-feed li { @apply grid grid-cols-[10px_1fr] gap-2; }
.event-sheet-feed li.event-group-title { grid-template-columns: minmax(0, 1fr); }
.event-sheet-feed li > i { @apply mt-1.5 size-[5px] rounded-full bg-[#b79550] shadow-[0_0_0_4px_rgba(183,149,80,.08)]; }
.event-sheet-feed span { @apply text-[11px] font-bold text-[var(--app-text)]; }
.event-sheet-feed p { @apply mt-0.5 text-[10px] leading-5 text-[var(--app-text-muted)]; }
.new-event-button { @apply mx-auto mt-2 border border-[var(--app-accent)] bg-[var(--app-surface-muted)] px-3 py-1.5 text-[10px] text-gold-light; }
</style>
