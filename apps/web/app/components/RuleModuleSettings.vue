<template>
  <fieldset class="waiting-rules">
    <legend>{{ isOwner ? '規則模組' : '啟用規則' }}</legend>
    <p>規則版本 {{ ruleVersion }}</p>
    <details v-if="isOwner">
      <summary>選擇規則版本</summary>
      <label v-for="version in (['5.17', '5.16'] as const)" :key="version">
        <input type="radio" name="waiting-rule-version" :value="version" :checked="ruleVersion === version" :disabled="disabled" @change="$emit('update:ruleVersion', version)">
        {{ version }}
      </label>
    </details>
    <section
      v-for="group in groups"
      :key="group.id"
      class="waiting-rule-group"
      :aria-labelledby="`waiting-rule-group-${group.id}`"
    >
      <h3 :id="`waiting-rule-group-${group.id}`">{{ group.label }}</h3>
      <label v-for="rule in group.rules" :key="rule.id" class="rule-toggle">
        <input
          type="checkbox"
          :aria-label="rule.label"
          :checked="enabledRuleModules.includes(rule.id)"
          :disabled="disabled || !isOwner"
          @change="toggle(rule.id)"
        >
        {{ rule.label }}
      </label>
    </section>
  </fieldset>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { modulesForVersion } from '#shared/utils/rule-versions'
import type { RuleModuleSpec } from '~/types/fewfc'
import { createRuleModulePolicy, presentationForRuleModule } from '#shared/utils/rule-modules'

const props = withDefaults(defineProps<{
  ruleVersion?: '5.16' | '5.17'
  catalog: RuleModuleSpec[]
  enabledRuleModules: string[]
  isOwner: boolean
  disabled?: boolean
}>(), {
  disabled: false,
  ruleVersion: '5.16',
})

const emit = defineEmits<{
  'update:ruleVersion': [ruleVersion: '5.16' | '5.17']
  'update:enabledRuleModules': [enabledRuleModules: string[]]
}>()

const policy = computed(() => createRuleModulePolicy(modulesForVersion(props.catalog, props.ruleVersion)))
const groupLabels = {
  optional: '選用規則',
  advanced: '進階規則',
  theme: '主題規則',
}
const groups = computed(() => (['optional', 'advanced', 'theme'] as const).map(id => ({
  id,
  label: groupLabels[id],
  rules: policy.value.modules
    .filter(module => module.category === id)
    .map(module => ({ id: module.id, ...presentationForRuleModule(module.id) })),
})))

function toggle(moduleId: string) {
  if (props.disabled || !props.isOwner) return
  const next = props.enabledRuleModules.includes(moduleId)
    ? policy.value.disable(props.enabledRuleModules, moduleId)
    : policy.value.enable(props.enabledRuleModules, moduleId)
  emit('update:enabledRuleModules', next)
}
</script>

<style scoped>
.waiting-rules { display: grid; gap: 14px; margin-top: 24px; padding: 0; border: 0; min-width: 0; }
.waiting-rules legend { padding: 0 0 10px; font: 700 18px / 1.5 var(--font-serif); }
.waiting-rules > p { margin: 0; font-size: 12px; color: var(--app-text-muted); }
.waiting-rules details { font-size: 12px; }
.waiting-rules summary { cursor: pointer; color: var(--app-accent-strong); }
.waiting-rules details label { display: inline-flex; align-items: center; gap: 6px; padding: 12px 16px 0 0; }
.waiting-rule-group { display: flex; flex-wrap: wrap; gap: 8px; }
.waiting-rule-group h3 { width: 100%; margin: 0 0 2px; font-size: 12px; font-weight: 500; color: var(--app-text-muted); }
.rule-toggle { display: inline-flex; align-items: center; gap: 8px; min-height: 36px; padding: 6px 10px; border: 1px solid var(--ui-glass-stroke); border-radius: 10px; background: color-mix(in srgb, var(--app-surface-muted) 76%, transparent); font-size: 12px; cursor: pointer; }
.rule-toggle:has(input:disabled) { cursor: default; }
.waiting-rules input { accent-color: var(--app-accent); }
.waiting-rules :is(input, summary):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
