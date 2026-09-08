import { expect, test } from 'bun:test'
import { modulesForVersion } from './rule-versions'
import type { RuleModuleSpec } from '../../app/types/fewfc'

const catalog: RuleModuleSpec[] = ['spirit', 'echo', 'tribulation', 'totem-formation'].map(id => ({
  id, category: 'theme', defaultEnabled: true, dependencies: [],
}))

test('theme controls only expose themes in the selected playable version', () => {
  expect(modulesForVersion(catalog, '5.16').map(module => module.id)).toEqual(['spirit', 'echo', 'tribulation'])
  expect(modulesForVersion(catalog, '5.17').map(module => module.id)).toEqual(['spirit', 'totem-formation'])
})
