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
          class="battlefield"
          :class="{
            'four-player': playerSeats.length === 4,
            'discard-open': discardOpen,
          }"
          aria-label="五行戰牌對戰桌"
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
                <small
                  v-for="(interpretation, index) in cardInterpretationsFor(seat.player)"
                  :key="`${seat.player}-interpretation-${index}`"
                  class="card-interpretation"
                >
                  {{ presentCardInterpretation(interpretation) }}
                </small>
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
              <button
                v-for="card in cardsFor(seat.player)"
                :key="card.id"
                type="button"
                class="playing-card"
                :class="[
                  { hidden: card.hidden, selected: card.selected },
                  elementClass(card.element),
                ]"
                :data-card-id="card.cardId"
                :disabled="!card.selectable || !roomConnected"
                :aria-label="card.hidden ? '牌背' : card.label"
                :aria-pressed="card.selected"
                @click="game.toggleCardSelection(seat.player, card.cardId)"
              >
                <span v-if="!card.hidden" class="card-level">{{ cardLevel(card.level) }}</span>
                <span v-if="!card.hidden" class="card-element">{{ cardElement(card.element) }}</span>
              </button>
            </div>
          </div>

          <div class="board-center">
            <div
              class="discard-piles"
              :class="{ personal: state.playerDiscards.length > 0 }"
              aria-label="棄牌堆"
            >
              <div
                v-for="pile in visibleDiscardPiles"
                :key="pile.owner ?? 'shared'"
                class="discard-pile"
                :class="[
                  { disabled: discardPileUnavailable(pile.cards) },
                  pile.position ? `discard-position-${pile.position}` : '',
                ]"
              >
                <span>{{ pile.owner ? `${playerLabel(pile.owner)} 棄牌` : '棄牌' }}</span>
                <button
                  class="discard-pile-trigger"
                  type="button"
                  aria-haspopup="dialog"
                  aria-controls="discard-composition"
                  :aria-expanded="discardOpen && activeDiscardOwner === pile.owner"
                  :aria-disabled="discardPileUnavailable(pile.cards)"
                  :aria-label="`查看${pile.owner ? `${playerLabel(pile.owner)}的` : ''}棄牌內容，共 ${pile.cards.length} 張`"
                  @click.stop="toggleDiscardComposition(pile.owner, $event)"
                >
                  {{ pile.cards.length }}
                </button>
              </div>
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
                    <span
                      v-for="card in previousFormationCards"
                      :key="card.id"
                      class="formation-card"
                      :class="[{
                        hidden: card.hidden,
                      }, elementClass(card.element)]"
                      :aria-label="card.hidden ? '牌背' : card.label"
                    >
                      <i v-if="!card.hidden">{{ cardLevel(card.level) }}</i>
                      <b v-if="!card.hidden">{{ cardElement(card.element) }}</b>
                    </span>
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
                      v-for="pouchAction in pouchStrategyActions"
                      :key="`pouch-${pouchAction.label}`"
                      type="button"
                      :title="pouchAction.detail || undefined"
                      :aria-label="labelWithDetail(pouchAction.label, pouchAction.detail)"
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
                      v-for="ability in directPlayableAbilities"
                      :key="playableAbilityKey(ability)"
                      type="button"
                      :title="playableActionDetail(ability) || undefined"
                      :aria-label="playableActionAccessibleLabel(ability)"
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
                        aria-haspopup="menu"
                        :aria-expanded="darkSpiritMenuOpen"
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
                        aria-haspopup="menu"
                        :aria-expanded="splendorMenuOpen"
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
                          @click="startSplendorAction(ability)"
                        >
                          {{ ability.declaredLevel }} 級
                        </button>
                      </div>
                    </div>
                    <button
                      v-if="game.interaction.value.canRetrieveDiscard"
                      class="retrieve-action"
                      type="button"
                      :title="discardRetrievalDetail || undefined"
                      :aria-label="labelWithDetail('棄牌回收', discardRetrievalDetail)"
                      :disabled="!roomConnected"
                      @mouseenter="showTextActionDetail(discardRetrievalDetail)"
                      @mouseleave="hideActionDetail"
                      @focus="showTextActionDetail(discardRetrievalDetail)"
                      @blur="hideActionDetail"
                      @click="game.retrievePreviousTurnDiscard()"
                    >
                      棄牌回收
                    </button>
                    <p v-if="!game.playableAbilities.value.length && !game.interaction.value.canRetrieveDiscard">
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
                      v-for="action in game.playableMainActions.value"
                      :key="`${action.type}:${action.id}:${action.cards.join('-')}:${action.type === 'performFormation' ? `${action.starSubstitution?.card ?? 'printed'}:${action.matchOption?.role ?? 'default'}:${action.matchOption?.card ?? ''}` : 'profession'}`"
                      type="button"
                      :title="playableActionDetail(action) || undefined"
                      :aria-label="playableActionAccessibleLabel(action)"
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
                      @click="game.passAction()"
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
              <p v-if="state.preparationPlayer">
                等待 {{ playerLabel(state.preparationPlayer) }} 完成選擇。
              </p>
              <p v-else>伺服器正在洗牌與發牌。</p>
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
                  @choose="game.togglePendingChoiceCard"
                />
                <div v-else class="choice-cards">
                  <button
                    v-for="card in state.pendingChoice.choice.cards"
                    :key="card.id"
                    type="button"
                    :class="{ selected: game.selectedChoiceCards.value.includes(card.id) }"
                    :aria-pressed="game.selectedChoiceCards.value.includes(card.id)"
                    :disabled="game.isLoading.value || !roomConnected
                      || (game.selectedChoiceCards.value.length >= state.pendingChoice.choice.maximum
                        && !game.selectedChoiceCards.value.includes(card.id))"
                    @click="game.togglePendingChoiceCard(card.id)"
                  >
                    {{ card.label }}
                  </button>
                </div>
                <p class="choice-count">
                  已選 {{ game.selectedChoiceCards.value.length }}
                  （{{ state.pendingChoice.choice.minimum }}–{{ state.pendingChoice.choice.maximum }}）
                </p>
                <button
                  class="choice-submit"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected || !game.canSubmitPendingChoice.value"
                  @click="game.submitPendingChoice()"
                >
                  確認選擇
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

          <div v-if="roomWaiting" class="waiting-overlay">
            <div class="waiting-panel">
              <section class="waiting-main">
                <h2>{{ activeRoomName }}</h2>
                <p>{{ waitingRoomSummary }}</p>
                <p v-if="game.lockedDeckName.value" class="muted">
                  本局使用：{{ game.lockedDeckName.value }}
                </p>
                <fieldset class="waiting-rules">
                  <legend>{{ isRoomOwner ? '規則模組' : '啟用規則' }}</legend>
                  <section
                    v-for="group in ruleGroups"
                    :key="group.id"
                    class="waiting-rule-group"
                    :aria-labelledby="`waiting-rule-group-${group.id}`"
                  >
                    <h3 :id="`waiting-rule-group-${group.id}`">{{ group.label }}</h3>
                    <label v-for="rule in group.rules" :key="rule.id" class="rule-toggle">
                      <input
                        type="checkbox"
                        :checked="onlineMetadata?.enabledRuleModules.includes(rule.id)"
                        :disabled="game.isLoading.value || !isRoomOwner"
                        @change="toggleWaitingRule(rule.id)"
                      >
                      {{ rule.label }}
                    </label>
                  </section>
                </fieldset>
                <p v-if="game.errorMessage.value" class="form-error" role="alert">
                  {{ game.errorMessage.value }}
                </p>
              </section>

              <aside class="waiting-side" aria-label="房間成員與操作">
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
              </aside>
            </div>
          </div>

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
  SpiritKind,
  StarKind,
  TeamId,
  ViewerId,
} from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '#shared/game-room'
import { createRuleModulePolicy, presentationForRuleModule } from '#shared/utils/rule-modules'
import { buildCardComposition, CARD_LEVELS } from '~/lib/card-composition'
import { cardElementClass, cardElementGlyph } from '~/lib/card-face-presentation'
import { presentCardInterpretation } from '~/lib/card-interpretation-presentation'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import { presentPersistentEffects } from '~/lib/persistent-effect-presentation'
import { presentDiscardRetrievalAction, presentPlayableAction, presentSecretStrategyOption } from '~/lib/action-detail-presentation'
import { splitEarthChoiceKey, usesSplitEarthFormationGroups } from '#shared/utils/split-earth-formation-choice'
import { roomRouteResult } from '~/lib/navigation'
import { chainChoiceAnswer, toggleChoiceCard } from '~/lib/pending-choice-interaction'
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
  options?: Parameters<typeof game.triggerSecretStrategy>[1]
}
const pouchStrategyActions = computed<PouchStrategyAction[]>(() => {
  if (viewer.value === 'observer' || !game.interaction.value.canTriggerPouch) return []
  const pouch = state.value.pouches.find(entry => entry.owner === viewer.value)?.card
  if (!pouch) return []
  const actions: PouchStrategyAction[] = []
  const requirements = game.interaction.value.secretStrategyOptions
    .filter(requirement => requirement.sourceCard === pouch.id)
  for (const requirement of requirements) {
    const { strategy } = requirement
    const detail = presentSecretStrategyOption(requirement)
    if (requirement.input === 'none') {
      actions.push({ label: `秘計‧${strategyLabel(strategy)}`, detail, strategy })
    } else if (requirement.input === 'targetPlayer') {
      for (const player of requirement.targetPlayers) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → ${playerLabel(player)}`,
          detail,
          strategy,
          options: { targetPlayer: player },
        })
      }
    } else if (requirement.input === 'deckDiscardSwap') {
      if (requirement.discardCards.length >= requirement.requiredCardCount) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)}`,
          detail,
          strategy,
        })
      }
    } else if (requirement.input === 'star') {
      for (const star of requirement.stars) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → ${starLabel(star)}`,
          detail,
          strategy,
          options: { star },
        })
      }
      for (const star of requirement.breakStars) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → 破除 ${starLabel(star)}`,
          detail,
          strategy,
          options: { star, breakStar: true },
        })
      }
    } else if (requirement.input === 'retreat') {
      actions.push({ label: `秘計‧${strategyLabel(strategy)} → 破除環境`, detail, strategy })
      for (const cardId of requirement.handCards) {
        const card = ownHandCards.value.find(candidate => candidate.id === cardId)
        if (card) {
          actions.push({
            label: `秘計‧${strategyLabel(strategy)} → 捨棄 ${card.label}`,
            detail,
            strategy,
            options: { discardCard: card.id },
          })
        }
      }
    }
  }
  return actions
})
const pouchChoiceKind = ref<'chain' | 'sheep' | null>(null)
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
  const prospectiveDeckCount = ownDeckCards.value.length - 2
  const sheepCanComplete = prospectiveDeckCount + ownDiscardCards.value.length >= 2
  return game.interaction.value.secretStrategyOptions
    .filter(option => option.sourceCard === card.id)
    .filter(option => option.strategy !== 'SheepStealing' || sheepCanComplete)
})
const selectedChainStrategyAction = computed(() => chainStrategyOptions.value.find(
  option => option.strategy === chainStrategySelection.value,
) ?? null)
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
  void game.triggerSecretStrategy(action.strategy, action.options)
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

const actionDetail = ref<string | null>(null)
const discardRetrievalDetail = computed(() => {
  const detail = game.interaction.value.discardRetrievalAction
  return detail ? presentDiscardRetrievalAction(detail) : ''
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
      || state.value.phase !== 'Main'
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
  && game.interaction.value.canPass
  && game.interaction.value.hasOptionalEffect
))
const gameResultText = computed(() => {
  if (state.value.fiveStarAlignment) {
    return `五星連珠 · ${teamLabel(state.value.fiveStarAlignment.team)} 勝利`
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

function toggleDiscardComposition(owner: PlayerId | null, event: MouseEvent) {
  if (discardOpen.value && activeDiscardOwner.value === owner) {
    closeDiscardComposition()
    return
  }

  const pile = visibleDiscardPiles.value.find(candidate => candidate.owner === owner)
  if (!pile || discardPileUnavailable(pile.cards)) {
    return
  }

  discardTrigger.value = event.currentTarget as HTMLButtonElement
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
    && viewer.value === state.value.currentPlayer
    && !state.value.pendingChoice
    && !game.isLoading.value
  ) {
    event.preventDefault()
    void game.advanceAutomatic()
    return
  }

  if (event.key !== 'Escape') {
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
}

async function toggleWaitingRule(moduleId: string) {
  const current = onlineMetadata.value?.enabledRuleModules ?? []
  const next = current.includes(moduleId)
    ? ruleModulePolicy.value.disable(current, moduleId)
    : ruleModulePolicy.value.enable(current, moduleId)
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
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: hand.cards.count }, (_, index) => ({
    id: `${player}-hidden-${index}`,
    cardId: -index - 1,
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
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: cards.count }, (_, index) => ({
    id: `${prefix}-hidden-${index}`,
    cardId: -index - 1,
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

function cardInterpretationsFor(player: PlayerId) {
  return state.value.cardInterpretations.filter(interpretation => interpretation.player === player)
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
    Main: '主要階段',
    MainPhase: '主要階段',
    TurnStart: '回合開始',
    TurnDraw: '回合抽牌',
    TurnDrawDiscardChoice: '回合抽牌',
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

function elementClass(element: Element | null | undefined): string {
  return cardElementClass(element)
}

function cardElement(element: Element | null | undefined): string {
  return cardElementGlyph(element)
}

function cardLevel(level: number | null | undefined): string {
  return level === null || level === undefined ? '◆' : String(level)
}


</script>

<style>
@reference "../../assets/css/main.css";

@scope (.game-page) {
:scope { @apply flex h-[calc(100vh-84px)] flex-col overflow-hidden max-[900px]:h-auto max-[900px]:overflow-visible; }
.back-button { @apply grid size-9 place-items-center border border-[#4a554e] bg-[rgba(17,23,19,.88)] text-base text-[#ddd7c9] hover:border-[#b99550] hover:text-gold-light; }
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
  background: radial-gradient(ellipse at center, #273029 0%, #141b17 58%, #0f1512 100%);
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
.profession-badge { @apply relative cursor-help border border-[#79633b] bg-[#18201b] px-1.5 py-0.5 text-[10px] text-gold-light outline-none focus-visible:border-[#d1ad62]; }
.profession-summary { @apply invisible absolute top-[calc(100%+6px)] left-0 z-20 grid w-64 gap-1 border border-[#64583f] bg-[#18201b] p-2.5 text-left opacity-0 shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.profession-summary b { @apply font-serif text-xs text-gold-light; }
.profession-summary small { @apply whitespace-normal text-[10px]! leading-4 text-muted!; }
.card-interpretation { @apply border border-[#526c7c] bg-[#17232b] px-1.5 py-0.5 text-[9px]! text-[#b9d5e5]!; }
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
.playing-card {
  width: clamp(62px, 7vw, 92px); aspect-ratio: 5 / 7; border: 1px solid #79715e; border-radius: 5px;
  --card-face: #ded8c8;
  --card-face-light: #f0eadc;
  --card-border: #79715e;
  --card-ink: #18201c;
  border-color: var(--card-border);
  background-color: var(--card-face);
  background-image: linear-gradient(145deg, var(--card-face-light), var(--card-face));
  color: var(--card-ink);
  @apply relative flex flex-col items-center justify-center p-2 transition-[.18s] max-[600px]:w-[58px];
  box-shadow: 0 5px 15px rgba(0,0,0,.35);
}
.playing-card:enabled:hover, .playing-card.selected { transform: translateY(-14px); border-color: #e2bd67; box-shadow: 0 0 0 2px #c9a451, 0 12px 18px rgba(0,0,0,.45); z-index: 5; }
.playing-card.hidden, .formation-card.hidden {
  background: repeating-linear-gradient(45deg, var(--card-back-base), var(--card-back-base) 5px, var(--card-back-stripe) 5px, var(--card-back-stripe) 10px);
  border: 2px solid var(--card-back-border);
}
.playing-card.hidden::after, .formation-card.hidden::after {
  content: "";
  width: 48%;
  aspect-ratio: 1;
  border: 1px solid rgba(198, 163, 94, .58);
  transform: rotate(45deg);
}
.card-level { @apply absolute top-1 left-1/2 -translate-x-1/2 font-serif text-xs leading-none font-extrabold; }
.card-element { @apply grid size-[35px] place-items-center rounded-full border border-current font-serif text-lg; color: var(--card-ink); }
.playing-card.element-Metal { --card-face: #ddd5b5; --card-face-light: #f3eed8; --card-border: #89783e; --card-ink: #67571e; }
.playing-card.element-Wood { --card-face: #cfe0ce; --card-face-light: #e8f1e5; --card-border: #52765a; --card-ink: #315f3d; }
.playing-card.element-Water { --card-face: #cbdfe8; --card-face-light: #e7f1f5; --card-border: #4d7890; --card-ink: #245d78; }
.playing-card.element-Fire { --card-face: #ead0c9; --card-face-light: #f6e7e2; --card-border: #985448; --card-ink: #8d3026; }
.playing-card.element-Earth { --card-face: #e4d5ba; --card-face-light: #f3ead8; --card-border: #936d3b; --card-ink: #785027; }
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
.discard-pile { @apply grid justify-items-center gap-1.5 text-[9px] text-[#707b73]; }
.discard-piles.personal .discard-pile { @apply pointer-events-auto absolute w-[90px]; }
.discard-position-top { top: 0; left: 0; }
.discard-position-left { bottom: 0; left: 0; }
.discard-position-right { top: 0; right: 0; }
.discard-position-bottom { right: 0; bottom: 0; }
.discard-pile-trigger { @apply grid w-[52px] place-items-center border border-[#665b44] bg-[#18201b] font-serif text-xl text-[#a68d56]; aspect-ratio: 5/7; }
.discard-pile-trigger { @apply p-0 hover:border-[#b99550] hover:text-gold-light; }
.discard-pile-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 3px; }
.discard-pile.disabled .discard-pile-trigger { @apply cursor-not-allowed opacity-45; }
.discard-composition-layer { @apply absolute right-0 z-20; bottom: calc(50% + 48px); }
.discard-composition {
  @apply w-[300px] border border-[#8e733d] bg-[#18201b] p-3.5 text-[#ece8dd] shadow-[0_18px_48px_rgba(0,0,0,.52)];
}
.discard-composition h2 { @apply mb-2.5 font-serif text-sm text-gold-light; }
.card-composition table { @apply w-full table-fixed border-collapse; }
.card-composition th, .card-composition td { @apply h-8 border border-[#354039] text-center; }
.card-composition thead th { @apply text-[10px] font-bold text-[#d5d8d4]; }
.card-composition tbody th { @apply w-7 text-[10px] font-normal text-muted; }
.card-composition td strong { @apply font-serif text-sm text-[#e4c47d]; }
.card-composition td.empty strong { @apply text-[#59635c]; }
.card-composition tbody tr:nth-child(1) > th { color: #ded5ba; }
.card-composition tbody tr:nth-child(2) > th { color: #77a980; }
.card-composition tbody tr:nth-child(3) > th { color: #75a8bd; }
.card-composition tbody tr:nth-child(4) > th { color: #d17a6c; }
.card-composition tbody tr:nth-child(5) > th { color: #c8a265; }
.pouch-composition { @apply mx-auto mt-5 w-[min(390px,calc(100vw-64px))] border border-[#8e733d] bg-[#18201b] p-3.5 text-[#ece8dd] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.pouch-composition td { @apply p-0; }
.pouch-composition td button { @apply grid size-full min-h-8 place-items-center border-0 bg-transparent text-[#e4c47d] hover:bg-[rgba(185,149,80,.16)] disabled:cursor-not-allowed disabled:opacity-45; }
.pouch-composition td button[aria-pressed="true"] { @apply bg-[rgba(185,149,80,.3)] shadow-[inset_0_0_0_2px_#d1ad62]; }
.choice-card-matrix td button small { @apply text-[8px] font-normal text-[#f0d99e]; }
.chain-composition { @apply mt-2; }
.choice-selection-summary { @apply mx-auto mb-1 flex max-w-[390px] items-center justify-between gap-3 text-xs text-gold-light; }
.choice-selection-summary button { @apply border border-[#665b44] bg-[#18201b] px-2 py-1 text-[10px] text-[#d5d8d4] hover:border-[#b99550]; }
.formation-field { @apply relative z-3 grid min-h-48 w-full min-w-0 grid-rows-[auto_1fr_auto] items-center border-x border-[rgba(166,141,86,.14)] px-3 py-2 text-center text-[10px] text-[#69736c]; grid-column: 2; grid-row: 1; }
.formation-field-heading { @apply flex flex-wrap items-center justify-center gap-2; }
.formation-field-label { @apply text-[#9a8251]; letter-spacing: .2em; }
.environment-badge { @apply border border-[#64583f] bg-[#1a211c] px-2 py-1 text-[9px] text-gold-light; }
.previous-formation { @apply grid min-h-24 content-center justify-items-center gap-1.5; }
.previous-formation small { @apply text-[9px] text-muted; }
.previous-formation strong { @apply font-serif text-sm text-gold-light; }
.previous-formation p { @apply text-[10px] text-[#68726b]; }
.formation-cards { @apply flex min-h-12 items-center justify-center; }
.formation-card { @apply relative grid w-8 place-items-center rounded-[3px] border border-[#79715e] bg-[#d8cfba] text-[#18201c]; aspect-ratio: 5/7; margin-left: -4px; }
.formation-card i { @apply absolute top-0.5 left-1 text-[8px] not-italic; }
.formation-card b { @apply font-serif text-xs; }
.turn-controls { @apply relative grid min-h-14 content-center gap-2 border-t border-[rgba(166,141,86,.14)] pt-2; }
.ability-panel, .action-panel { @apply grid gap-1.5 border border-[rgba(166,141,86,.18)] bg-[rgba(17,23,19,.45)] p-2; }
.ability-panel header, .action-panel header { @apply flex flex-wrap items-baseline justify-between gap-x-2 text-left; }
.ability-panel h3, .action-panel h3 { @apply font-serif text-xs text-gold-light; }
.ability-panel small, .action-panel small { @apply text-[9px] text-muted; }
.action-candidates { @apply flex max-w-full flex-wrap justify-center gap-1.5; }
.action-candidates button { @apply min-h-8 border border-[#4c554f] bg-[#18201b] px-2.5 py-1.5 text-[10px] text-[#e1ddd2] hover:border-[#b99550]; }
.action-candidates p { @apply text-[9px] text-[#68726b]; }
.spirit-level-picker { @apply relative; }
.spirit-level-trigger { @apply grid min-h-8 min-w-12 place-items-center border border-[#4c554f] bg-[#18201b] px-2.5 py-1.5 text-[10px] text-[#e1ddd2] hover:border-[#b99550]; }
.spirit-level-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 2px; }
.spirit-level-options { @apply absolute bottom-[calc(100%+5px)] left-1/2 z-10 grid min-w-20 -translate-x-1/2 gap-1 border border-[#64583f] bg-[#121915] p-1 shadow-[0_10px_24px_rgba(0,0,0,.45)]; }
.action-candidates .spirit-level-options button { @apply min-h-7 whitespace-nowrap px-2 py-1; }
.action-candidates .skip-action { @apply border-[#79633b] text-gold-light; }
.action-processing { @apply text-[#d0aa5e]; }
.action-error { @apply text-[#d79587]; }
.action-prompt { @apply text-[#68726b]; }
.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[#64583f] bg-[#1c241f] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.action-detail strong { @apply mr-2 text-gold-light; }
.choice-overlay,
.choice-waiting-overlay { @apply absolute inset-0 z-12 grid place-items-center bg-[rgba(7,10,8,.28)] text-center; }
.choice-overlay > div,
.choice-waiting-overlay > div { @apply max-h-[calc(100%-32px)] min-w-90 overflow-y-auto border border-[#8e733d] bg-[rgba(24,32,27,.94)] p-[30px] shadow-[0_18px_48px_rgba(0,0,0,.42)]; }
.choice-overlay h2,
.choice-waiting-overlay h2 { @apply mt-2.5 mb-5 font-serif; }
.virtual-formation-card-dialog { @apply max-w-[min(560px,calc(100vw-32px))]; }
.virtual-formation-card-matrix { @apply mx-auto border-collapse text-xs; }
.virtual-formation-card-matrix th { @apply border border-[#4f584f] bg-[#18201b] px-2 py-1.5 font-normal text-muted; }
.virtual-formation-card-matrix tbody th { @apply min-w-14 text-gold-light; }
.virtual-formation-card-matrix td { @apply border border-[#4f584f] p-0; }
.virtual-formation-card-option { @apply grid size-11 place-items-center bg-[#222b25] font-serif text-sm text-[#e5dfd1] hover:bg-[#3a443d] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.virtual-formation-card-option:focus-visible { @apply relative z-1 outline-2 outline-offset-[-3px] outline-[#d1ad62]; }
.virtual-formation-card-cancel { @apply mt-5 min-h-9 border border-[#59635c] bg-[#18201b] px-4 py-2 text-xs text-[#d5d8d4] hover:border-[#b99550] hover:text-gold-light; }
.choice-cards { @apply flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-cards button { @apply border border-[#ae8b47] bg-[#ede6d4] p-2.5 text-[#18201c]; }
.choice-cards button.selected { @apply bg-[#c9a451] font-bold shadow-[0_0_0_2px_#f0d99e]; }
.choice-options { @apply mt-3 flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-options button { @apply min-h-10 border border-[#59635c] bg-[#18201b] px-3 py-2 text-xs text-[#d5d8d4] hover:border-[#b99550] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.choice-options button:focus-visible { @apply border-[#d1ad62] outline-2 outline-offset-2 outline-[#d1ad62]; }
.choice-options button.selected,
.choice-options button[aria-pressed="true"] { @apply border-[#d1ad62] bg-[#c9a451] font-bold text-[#121713] shadow-[0_0_0_2px_#f0d99e]; }
.choice-actions { @apply mt-5; }
.choice-actions .choice-confirm { @apply border-[#b99550] bg-[#b99550] font-bold text-[#121713] hover:bg-[#c9a451] hover:text-[#121713]; }
.choice-count { @apply mt-4 text-xs text-muted; }
.choice-submit { @apply mt-3 border border-[#b99550] bg-[#b99550] px-5 py-2 text-xs font-bold text-[#121713] disabled:cursor-not-allowed disabled:opacity-45; }
.setup-reveal { @apply absolute inset-0 z-15 grid place-items-center bg-[rgba(7,10,8,.88)] text-center backdrop-blur-[5px]; }
.setup-reveal > div { @apply grid w-[min(520px,calc(100vw-32px))] gap-4 border border-[#b99550] bg-[#18201b] p-7; }
.setup-reveal h2 { @apply font-serif text-2xl text-gold-light; }
.setup-reveal ol { @apply m-0 grid list-none grid-cols-4 gap-2 p-0 max-[600px]:grid-cols-2; counter-reset: order; }
.setup-reveal li { @apply border border-[#39443d] bg-[#111713] p-2 text-xs; counter-increment: order; }
.setup-reveal li::before { content: counter(order) ". "; color: #c6a35e; }
.revealed-teams { @apply grid grid-cols-2 gap-3; }
.revealed-teams span { @apply grid gap-1 border border-[#39443d] p-3 text-xs text-muted; }
.revealed-teams strong { @apply text-gold-light; }
.waiting-overlay { @apply absolute inset-0 flex items-start justify-center overflow-y-auto bg-[rgba(7,10,8,.78)] py-4 backdrop-blur-[4px]; z-index: 13; }
.waiting-panel { width: min(860px, calc(100% - 64px)); @apply my-auto grid grid-cols-[minmax(0,1.45fr)_minmax(240px,.75fr)] gap-6 border border-[#8e733d] bg-[#18201b] p-7 text-left shadow-[0_24px_80px_rgba(0,0,0,.42)]; }
.waiting-main { @apply min-w-0; }
.waiting-side { @apply grid min-w-0 content-start gap-4 border-l border-[#354039] pl-6; }
.waiting-overlay h2 { @apply font-serif text-3xl text-gold-light; }
.waiting-overlay p:not(.section-kicker) { @apply text-sm text-muted; }
.waiting-members { @apply grid grid-cols-2 gap-3; }
.waiting-members span { @apply grid gap-1 border border-[#354039] bg-[#111713] p-3 text-sm text-muted; }
.waiting-members span.joined { @apply border-[#b99550] text-[#ece8dd]; }
.waiting-members small { @apply text-[10px] text-muted; }
.waiting-members button { @apply mt-1 border-0 bg-transparent text-[9px] text-[#c98e82]; }
.waiting-side .result-actions { @apply mt-0; }
.invite-link { @apply justify-self-start border-0 bg-transparent text-xs text-gold-light; }
.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }
.result-actions .ghost-button { @apply border-[#59635c] text-[#ece8dd]; }
.result-actions .primary-button { @apply justify-between; }

.game-sidebar { @apply grid min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden border-l border-line bg-panel max-[900px]:border-l-0; }
.game-sidebar.finished { grid-template-rows: auto minmax(0, 1fr); }
.result-panel { @apply border-b border-[#8e733d] bg-[#18201b] p-5; }
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
.event-feed span { color: #d4d8d4; font-size: 10px; font-weight: 700; }
.event-feed p { color: #6f7972; font-size: 9px; line-height: 1.45; margin-top: 2px; }
@media (min-width: 901px) {
  .player-identity strong { font-size: 14px; }
  .player-identity small { font-size: 12px; }
  .reconnecting-label, .turn-badge, .counter-badge, .shield-badge, .status-badge, .side-hand-count { font-size: 11px!important; }
  .discard-pile { font-size: 11px; }
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
    @apply fixed inset-0 grid place-items-center bg-[rgba(7,10,8,.72)] p-4 backdrop-blur-[3px];
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
  .discard-piles.personal .discard-pile { width: 48px; }
  .discard-pile-trigger { width: 38px; }
  .playing-card { width: 54px; }
  .seat-top .playing-card { width: 43px; }
  .player-identity strong { max-width: 110px; }
  .turn-badge { font-size: 8px!important; }
  .action-candidates button { min-height: 30px; padding: 4px 7px; }
  .waiting-panel { min-width: 0; width: calc(100vw - 32px); grid-template-columns: 1fr; gap: 18px; padding: 22px 16px; }
  .waiting-side { @apply border-t border-l-0 pt-4 pl-0; }
  .waiting-members { grid-template-columns: 1fr 1fr; }
}
}
</style>
