<template>
  <div
    class="card-composition pouch-composition choice-card-matrix"
    role="group"
    :aria-label="label"
  >
    <table>
      <caption class="sr-only">{{ caption }}</caption>
      <thead>
        <tr>
          <th scope="col"><span class="sr-only">等級</span></th>
          <th v-for="element in CARD_ELEMENTS" :key="element" scope="col">
            {{ element }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in composition" :key="row.level">
          <th scope="row">{{ row.level }}</th>
          <td
            v-for="cell in row.cells"
            :key="`${cell.element}-${cell.level}`"
            :class="{ empty: cell.count === 0 }"
          >
            <button
              v-if="cell.count > 0"
              type="button"
              :disabled="cellDisabled(cell.cardIds)"
              :aria-pressed="selectedCount(cell.cardIds) > 0"
              :aria-label="cellLabel(cell.element, cell.level, cell.cardIds)"
              @click="selectCell(cell.cardIds)"
            >
              <strong aria-hidden="true">{{ cell.count }}</strong>
              <small v-if="selectedCount(cell.cardIds) > 0" aria-hidden="true">
                已選 {{ selectedCount(cell.cardIds) }}
              </small>
            </button>
            <strong v-else aria-hidden="true">0</strong>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import type { CardInstanceId, PublicCard } from '~/types/fewfc'
import {
  buildCardComposition,
  cardForCompositionSelection,
  CARD_ELEMENTS,
  type CardCompositionSelectionMode,
} from '~/lib/card-composition'

const props = withDefaults(defineProps<{
  cards: PublicCard[]
  selectedCards?: CardInstanceId[]
  maximum?: number
  mode?: CardCompositionSelectionMode
  disabled?: boolean
  label: string
  caption: string
  actionLabel: string
}>(), {
  selectedCards: () => [],
  maximum: 1,
  mode: 'replace',
  disabled: false,
})

const emit = defineEmits<{
  select: [card: CardInstanceId]
}>()

const composition = computed(() => buildCardComposition(props.cards))

function selectedCount(cardIds: CardInstanceId[]): number {
  return cardIds.filter(card => props.selectedCards.includes(card)).length
}

function cellDisabled(cardIds: CardInstanceId[]): boolean {
  return props.disabled || (
    props.mode === 'toggle'
    && props.selectedCards.length >= props.maximum
    && selectedCount(cardIds) === 0
  )
}

function cellLabel(element: string, level: number, cardIds: CardInstanceId[]): string {
  const selected = selectedCount(cardIds)
  return `${props.actionLabel} ${element} ${level}，共 ${cardIds.length} 張，已選 ${selected} 張`
}

function selectCell(cardIds: CardInstanceId[]) {
  const card = cardForCompositionSelection(
    cardIds,
    props.selectedCards,
    props.maximum,
    props.mode,
  )
  if (card !== undefined) emit('select', card)
}
</script>
