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
