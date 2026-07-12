import { expect, test } from 'bun:test'
import type { RuleModuleSpec } from '../types/fewfc'
import { presentRoomRuleDifferences, ruleModuleLabel } from '../../shared/utils/ruleset-presentation'

const catalog: RuleModuleSpec[] = [
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'spirit', category: 'theme', defaultEnabled: true, dependencies: ['star'] },
  { id: 'pouch', category: 'theme', defaultEnabled: true, dependencies: ['spirit'] },
]

test('hides all-enabled room summary and names only disabled Rule Modules', () => {
  expect(presentRoomRuleDifferences(catalog, ['star', 'spirit', 'pouch'])).toBe('')
  expect(presentRoomRuleDifferences(catalog, ['star'])).toBe('停用：精靈、錦囊')
})

test('module names omit category prefixes already supplied by the group', () => {
  expect(ruleModuleLabel('star')).toBe('星辰圖記')
  expect(ruleModuleLabel('spirit')).toBe('精靈')
})
