<template>
  <div class="split-earth-formation-choice">
    <div
      v-if="!selectedGroup"
      class="choice-options"
      aria-label="選擇提供陣法的規則"
    >
      <button
        v-for="group in groups"
        :key="group.ruleModuleId ?? 'base-ruleset'"
        type="button"
        :disabled="disabled"
        :aria-label="`選擇規則 ${splitEarthRuleLabel(group.ruleModuleId)}`"
        @click="selectedRuleModuleId = group.ruleModuleId"
      >
        {{ splitEarthRuleLabel(group.ruleModuleId) }}
      </button>
    </div>

    <template v-else>
      <p class="choice-selection-summary">
        <strong>{{ splitEarthRuleLabel(selectedGroup.ruleModuleId) }}</strong>
        <button
          type="button"
          :disabled="disabled"
          @click="selectedRuleModuleId = undefined"
        >
          重新選擇規則
        </button>
      </p>
      <div class="choice-options" aria-label="選擇陣法">
        <button
          v-for="formation in selectedGroup.formations"
          :key="formation.id"
          type="button"
          :disabled="disabled"
          :aria-label="`選擇陣法 ${formation.name}`"
          @click="emit('select', formation.id)"
        >
          {{ formation.name }}
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { FormationChoiceGroup } from '~/types/fewfc'
import { splitEarthRuleLabel } from '#shared/utils/split-earth-formation-choice'

const props = withDefaults(defineProps<{
  groups: FormationChoiceGroup[]
  disabled?: boolean
}>(), {
  disabled: false,
})

const emit = defineEmits<{
  select: [formationId: string]
}>()

const selectedRuleModuleId = ref<string | null | undefined>(undefined)
const selectedGroup = computed(() => props.groups.find(
  group => group.ruleModuleId === selectedRuleModuleId.value,
) ?? null)
</script>
