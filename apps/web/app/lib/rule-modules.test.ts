import { expect, test } from 'bun:test'
import type { RuleModuleSpec } from '../types/fewfc'
import {
  createRuleModulePolicy,
  presentationForRuleModule,
} from '../../shared/utils/rule-modules'

const catalog: RuleModuleSpec[] = [
  { id: 'base-a', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'base-b', category: 'advanced', defaultEnabled: true, dependencies: [] },
  {
    id: 'theme-a',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['base-a', 'base-b'],
  },
  { id: 'theme-b', category: 'theme', defaultEnabled: false, dependencies: ['theme-a'] },
]

test('policy interprets defaults and dependencies from the supplied Rust catalog', () => {
  const policy = createRuleModulePolicy(catalog)

  expect(policy.defaults).toEqual(['base-a', 'base-b', 'theme-a'])
  expect(policy.normalize(['theme-a'])).toEqual([])
  expect(policy.normalize(['base-a', 'base-b', 'theme-a', 'unknown'])).toEqual([
    'base-a',
    'base-b',
    'theme-a',
  ])
  expect(policy.hasValidDependencies(['theme-a'])).toBe(false)
})

test('generic operations add transitive requirements and remove dependents', () => {
  const policy = createRuleModulePolicy(catalog)

  expect(policy.enable([], 'theme-b')).toEqual(['base-a', 'base-b', 'theme-a', 'theme-b'])
  expect(policy.disable(['base-a', 'base-b', 'theme-a', 'theme-b'], 'base-b')).toEqual(['base-a'])
})

test('presentation metadata is separate from authoritative rule policy', () => {
  expect(presentationForRuleModule('spirit')).toEqual({
    label: '精靈',
  })
  expect(presentationForRuleModule('future-module')).toEqual({
    label: 'future-module',
  })
})
