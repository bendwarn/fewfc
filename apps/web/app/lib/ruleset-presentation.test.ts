import assert from 'node:assert/strict'
import { test } from 'bun:test'
import type { RuleModuleSpec } from '../types/fewfc'
import { presentRoomRuleDifferences, ruleModuleLabel } from '../../shared/utils/ruleset-presentation'

const catalog: RuleModuleSpec[] = [
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'spirit', category: 'theme', defaultEnabled: true, dependencies: ['star'] },
  { id: 'pouch', category: 'theme', defaultEnabled: true, dependencies: ['spirit'] },
]

test('hides all-enabled room summary and names only disabled Rule Modules', () => {
  assert.equal(presentRoomRuleDifferences(catalog, ['star', 'spirit', 'pouch']), '')
  assert.equal(presentRoomRuleDifferences(catalog, ['star']), '停用：精靈、錦囊')
})

test('module names omit category prefixes already supplied by the group', () => {
  assert.equal(ruleModuleLabel('star'), '星辰圖記')
  assert.equal(ruleModuleLabel('spirit'), '精靈')
})
