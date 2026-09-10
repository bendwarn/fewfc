<template>
  <section class="chain-choice">
    <h3>錦囊給予對象</h3>
    <p v-if="pouchOwners.length === 1" class="choice-selection-summary">
      錦囊給予：{{ playerLabel(pouchOwner ?? pouchOwners[0] ?? '') }}
    </p>
    <div v-else class="choice-options" aria-label="選擇錦囊持有者">
      <button
        v-for="player in pouchOwners"
        :key="`pouch-owner-${player}`"
        type="button"
        :class="{ selected: pouchOwner === player }"
        :aria-pressed="pouchOwner === player"
        @click="emit('select-pouch-owner', player)"
      >
        {{ playerLabel(player) }}
      </button>
    </div>

    <h3>選擇錦囊牌</h3>
    <p v-if="pouchCard" class="choice-selection-summary">
      已選：{{ pouchCard.label }}
      <button type="button" @click="emit('clear-pouch-card')">重新選擇</button>
    </p>
    <CardChoiceMatrix
      class="chain-composition"
      :cards="pouchCards"
      :selected-cards="pouchCard ? [pouchCard.id] : []"
      label="連環錦囊牌組矩陣"
      caption="依五行與等級選擇連環錦囊牌"
      action-label="選擇作為連環錦囊"
      @choose="emit('choose-pouch-card', $event)"
    />

    <template v-if="pouchCard">
      <h3>選擇觸發牌（可選）</h3>
      <p class="choice-selection-summary">觸發牌須與錦囊牌的五行、等級都不同；不符合的格子無法選擇。</p>
      <p v-if="triggerCard" class="choice-selection-summary">
        已選：{{ triggerCard.label }}
        <button type="button" @click="emit('clear-trigger-card')">不觸發秘計</button>
      </p>
      <CardChoiceMatrix
        class="chain-composition"
        :cards="triggerCards"
        :selected-cards="triggerCard ? [triggerCard.id] : []"
        :disabled-card-ids="disabledTriggerCardIds"
        label="連環觸發牌組矩陣"
        caption="依五行與等級選擇連環觸發牌"
        action-label="選擇作為連環觸發牌"
        @choose="emit('choose-trigger-card', $event)"
      />
    </template>

    <template v-if="triggerCard">
      <h3>觸發秘計</h3>
      <div class="choice-options" aria-label="選擇秘計">
        <button
          v-for="option in strategyOptions"
          :key="`chain-strategy-${secretStrategyOptionStrategy(option)}`"
          type="button"
          :class="{ selected: selectedStrategy === secretStrategyOptionStrategy(option) }"
          :aria-pressed="selectedStrategy === secretStrategyOptionStrategy(option)"
          @click="emit('select-strategy', secretStrategyOptionStrategy(option))"
        >
          {{ strategyLabel(secretStrategyOptionStrategy(option)) }}
        </button>
      </div>

      <p v-if="selectedStrategyAction" class="action-detail">
        {{ presentSecretStrategyOption(selectedStrategyAction) }}
      </p>

      <div
        v-if="selectedStrategyAction?.type === 'targetPlayer'"
        class="choice-options"
        aria-label="離山目標"
      >
        <button
          v-for="player in selectedStrategyAction.targetPlayers"
          :key="`strategy-target-${player}`"
          type="button"
          :class="{ selected: selectedTarget === player }"
          :aria-pressed="selectedTarget === player"
          @click="emit('select-target', player)"
        >
          {{ playerLabel(player) }}
        </button>
      </div>

      <p v-if="selectedStrategyAction?.type === 'sheepStealing'">
        確認後會先依當前牌組狀態洗棄牌（若需要），再選擇牽羊交換的牌。
      </p>

      <template v-if="selectedStrategyAction?.type === 'star'">
        <h3>瞞天：取得星辰效果或破除星辰</h3>
        <div class="choice-options" aria-label="瞞天選擇">
          <button
            v-for="star in selectedStrategyAction.gainStars"
            :key="`strategy-star-${star}`"
            type="button"
            :class="{ selected: selectedStar === star && !breakStar }"
            :aria-pressed="selectedStar === star && !breakStar"
            @click="emit('select-star', star, false)"
          >
            取得 {{ starLabel(star) }}
          </button>
          <button
            v-for="star in selectedStrategyAction.breakStars"
            :key="`strategy-break-star-${star}`"
            type="button"
            :class="{ selected: selectedStar === star && breakStar }"
            :aria-pressed="selectedStar === star && breakStar"
            @click="emit('select-star', star, true)"
          >
            破除 {{ starLabel(star) }}
          </button>
        </div>
      </template>

      <template v-if="selectedStrategyAction?.type === 'environment'">
        <h3>走為：破除環境或捨棄手牌</h3>
        <div class="choice-options" aria-label="走為選擇">
          <button
            type="button"
            :class="{ selected: selectedRetreatCard === null }"
            :aria-pressed="selectedRetreatCard === null"
            @click="emit('select-retreat-card', null)"
          >
            破除環境
          </button>
          <button
            v-for="card in handCards"
            :key="`strategy-hand-${card.id}`"
            type="button"
            :class="{ selected: selectedRetreatCard === card.id }"
            :aria-pressed="selectedRetreatCard === card.id"
            @click="emit('select-retreat-card', card.id)"
          >
            捨棄 {{ card.label }}
          </button>
        </div>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import type {
  CardInstanceId,
  PlayerId,
  PublicCard,
  SecretStrategy,
  SecretStrategyOption,
  StarKind,
} from '~/types/fewfc'
import { presentSecretStrategyOption } from '~/lib/action-detail-presentation'
import { secretStrategyOptionStrategy } from '~/lib/secret-strategy-draft'
import CardChoiceMatrix from './CardChoiceMatrix.vue'

const props = defineProps<{
  pouchOwners: PlayerId[]
  pouchOwner: PlayerId | null
  pouchCards: PublicCard[]
  pouchCard: PublicCard | null
  triggerCards: PublicCard[]
  triggerCard: PublicCard | null
  strategyOptions: SecretStrategyOption[]
  selectedStrategy: SecretStrategy | null
  selectedStrategyAction: SecretStrategyOption | null
  selectedTarget: PlayerId | null
  selectedStar: StarKind | null
  breakStar: boolean
  selectedRetreatCard: CardInstanceId | null
  handCards: PublicCard[]
  playerLabel: (player: PlayerId) => string
  strategyLabel: (strategy: SecretStrategy) => string
  starLabel: (star: StarKind) => string
}>()

const disabledTriggerCardIds = computed(() => {
  if (!props.pouchCard) return []
  return props.triggerCards
    .filter(card => card.element === props.pouchCard?.element || card.level === props.pouchCard?.level)
    .map(card => card.id)
})

const emit = defineEmits<{
  'select-pouch-owner': [player: PlayerId]
  'choose-pouch-card': [card: CardInstanceId]
  'clear-pouch-card': []
  'choose-trigger-card': [card: CardInstanceId]
  'clear-trigger-card': []
  'select-strategy': [strategy: SecretStrategy]
  'select-target': [player: PlayerId]
  'select-star': [star: StarKind, breakStar: boolean]
  'select-retreat-card': [card: CardInstanceId | null]
}>()
</script>

<style scoped>
/* CardChoiceMatrix 會隨本元件進入 Teleport 的 modal；間距由使用情境擁有。 */
.chain-choice :deep(.chain-composition) { margin-top: .5rem; }
</style>
