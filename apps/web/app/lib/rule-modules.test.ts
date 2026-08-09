import { expect, test } from 'bun:test'
import type { RuleModuleSpec } from '../types/fewfc'
import {
  createRuleModulePolicy,
  presentationForRuleModule,
} from '../../shared/utils/rule-modules'

const catalog: RuleModuleSpec[] = [
  { id: 'discard-retrieval', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'personal-deck', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'five-directions-legend', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'hero-schools', category: 'advanced', defaultEnabled: true, dependencies: [] },
  {
    id: 'spirit',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'jianghu',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'confluence-generation',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  { id: 'dark-glimmer', category: 'theme', defaultEnabled: true, dependencies: ['spirit'] },
  {
    id: 'echo',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'tribulation',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  { id: 'pouch', category: 'theme', defaultEnabled: true, dependencies: ['personal-deck', 'spirit'] },
]

test('policy interprets defaults and dependencies from the supplied Rust catalog', () => {
  const policy = createRuleModulePolicy(catalog)

  expect(policy.defaults).toStrictEqual([
    'discard-retrieval',
    'personal-deck',
    'five-directions-legend',
    'star',
    'hero-schools',
    'spirit',
    'jianghu',
    'confluence-generation',
    'dark-glimmer',
    'echo',
    'tribulation',
    'pouch',
  ])
  expect(policy.normalize(['echo'])).toStrictEqual([])
  expect(policy.normalize(['star', 'five-directions-legend', 'hero-schools', 'echo', 'unknown'])).toStrictEqual([
    'five-directions-legend',
    'star',
    'hero-schools',
    'echo',
  ])
  expect(policy.hasValidDependencies(['echo'])).toBe(false)
})

test('generic operations add transitive requirements and remove dependents', () => {
  const policy = createRuleModulePolicy(catalog)

  expect(policy.enable([], 'pouch')).toStrictEqual([
    'personal-deck',
    'five-directions-legend',
    'star',
    'hero-schools',
    'spirit',
    'pouch',
  ])
  expect(policy.disable(policy.defaults, 'spirit')).toStrictEqual([
    'discard-retrieval',
    'personal-deck',
    'five-directions-legend',
    'star',
    'hero-schools',
    'jianghu',
    'confluence-generation',
    'echo',
    'tribulation',
  ])
})

test('presentation metadata is separate from authoritative rule policy', () => {
  expect(presentationForRuleModule('spirit')).toStrictEqual({
    label: '精靈',
  })
  expect(presentationForRuleModule('future-module')).toStrictEqual({
    label: 'future-module',
  })
})
