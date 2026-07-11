import assert from 'node:assert/strict'
import { test } from 'node:test'
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

  assert.deepEqual(policy.defaults, ['base-a', 'base-b', 'theme-a'])
  assert.deepEqual(policy.normalize(['theme-a']), [])
  assert.deepEqual(policy.normalize(['base-a', 'base-b', 'theme-a', 'unknown']), [
    'base-a',
    'base-b',
    'theme-a',
  ])
  assert.equal(policy.hasValidDependencies(['theme-a']), false)
})

test('generic operations add transitive requirements and remove dependents', () => {
  const policy = createRuleModulePolicy(catalog)

  assert.deepEqual(policy.enable([], 'theme-b'), ['base-a', 'base-b', 'theme-a', 'theme-b'])
  assert.deepEqual(
    policy.disable(['base-a', 'base-b', 'theme-a', 'theme-b'], 'base-b'),
    ['base-a'],
  )
})

test('presentation metadata is separate from authoritative rule policy', () => {
  assert.deepEqual(presentationForRuleModule('spirit'), {
    label: '精靈',
  })
  assert.deepEqual(presentationForRuleModule('future-module'), {
    label: 'future-module',
  })
})
