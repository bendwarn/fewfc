import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  hasValidServerRuleModuleDependencies,
  normalizeServerRuleModules,
} from '../../server/utils/rule-modules'
import { normalizeRuleModules } from '../../shared/game-room'
import {
  disableRuleModule,
  enableRuleModule,
  RULE_MODULE_SPECS,
} from '../../shared/utils/rule-modules'

test('server and room Rule Module allowlists stay aligned', () => {
  assert.deepEqual(normalizeServerRuleModules(undefined), normalizeRuleModules(undefined))
  assert.deepEqual(
    normalizeServerRuleModules(['star', 'five-directions-legend', 'unknown']),
    normalizeRuleModules(['star', 'five-directions-legend', 'unknown']),
  )
})

test('one shared catalog owns labels, defaults, and dependency metadata', () => {
  assert.deepEqual(
    RULE_MODULE_SPECS.find(module => module.id === 'spirit'),
    {
      id: 'spirit',
      label: '主題規則‧精靈',
      group: 'theme',
      defaultEnabled: true,
      dependencies: ['star', 'five-directions-legend', 'hero-schools'],
    },
  )
})

test('Hero Schools is available and enabled by default for new rooms', () => {
  assert.equal(normalizeRuleModules(['hero-schools']).includes('hero-schools'), true)
  assert.equal(normalizeServerRuleModules(['hero-schools']).includes('hero-schools'), true)
  assert.equal(normalizeRuleModules(undefined).includes('hero-schools'), true)
  assert.equal(normalizeServerRuleModules(undefined).includes('hero-schools'), true)
})

test('stored room module lists remain Hero-disabled when they omit Hero Schools', () => {
  assert.equal(normalizeRuleModules(['star']).includes('hero-schools'), false)
  assert.equal(normalizeServerRuleModules(['star']).includes('hero-schools'), false)
})

test('Spirit is default-on and requires every Advanced Rule Module', () => {
  assert.equal(normalizeRuleModules(undefined).includes('spirit'), true)
  assert.equal(normalizeServerRuleModules(undefined).includes('spirit'), true)
  assert.equal(normalizeRuleModules(['spirit']).includes('spirit'), false)
  assert.equal(normalizeServerRuleModules(['spirit']).includes('spirit'), false)
  assert.equal(hasValidServerRuleModuleDependencies(['spirit']), false)
  const complete = ['star', 'hero-schools', 'five-directions-legend', 'spirit']
  assert.equal(normalizeRuleModules(complete).includes('spirit'), true)
  assert.equal(normalizeServerRuleModules(complete).includes('spirit'), true)
  assert.equal(hasValidServerRuleModuleDependencies(complete), true)
})

test('Jianghu is default-on and requires every Advanced Rule Module', () => {
  const spec = RULE_MODULE_SPECS.find(module => module.id === 'jianghu')
  assert.deepEqual(spec, {
    id: 'jianghu',
    label: '主題規則‧江湖',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  })
  assert.equal(normalizeRuleModules(undefined).includes('jianghu'), true)
  assert.equal(normalizeRuleModules(['jianghu']).includes('jianghu'), false)
  const complete = ['star', 'hero-schools', 'five-directions-legend', 'jianghu']
  assert.equal(hasValidServerRuleModuleDependencies(complete), true)
})

test('Confluence Generation is default-on and requires every Advanced Rule Module', () => {
  const spec = RULE_MODULE_SPECS.find(module => module.id === 'confluence-generation')
  assert.deepEqual(spec, {
    id: 'confluence-generation',
    label: '主題規則‧匯流世代',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  })
  assert.equal(normalizeRuleModules(undefined).includes('confluence-generation'), true)
  assert.equal(normalizeRuleModules(['confluence-generation']).includes('confluence-generation'), false)
})

test('Dark Glimmer is default-on and depends transitively on Spirit', () => {
  const spec = RULE_MODULE_SPECS.find(module => module.id === 'dark-glimmer')
  assert.deepEqual(spec, {
    id: 'dark-glimmer',
    label: '主題規則‧黑暗微光',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['spirit'],
  })
  assert.equal(normalizeRuleModules(undefined).includes('dark-glimmer'), true)
  assert.equal(normalizeRuleModules(['dark-glimmer']).includes('dark-glimmer'), false)
  assert.deepEqual(enableRuleModule([], 'dark-glimmer'), [
    'star',
    'hero-schools',
    'five-directions-legend',
    'spirit',
    'dark-glimmer',
  ])
})

test('Echo is default-on and requires every Advanced Rule Module', () => {
  const spec = RULE_MODULE_SPECS.find(module => module.id === 'echo')
  assert.deepEqual(spec, {
    id: 'echo',
    label: '主題規則‧迴響',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  })
  assert.equal(normalizeRuleModules(undefined).includes('echo'), true)
  assert.equal(normalizeRuleModules(['echo']).includes('echo'), false)
  assert.deepEqual(enableRuleModule([], 'echo'), [
    'star',
    'hero-schools',
    'five-directions-legend',
    'echo',
  ])
  assert.deepEqual(
    disableRuleModule(
      ['star', 'hero-schools', 'five-directions-legend', 'echo'],
      'hero-schools',
    ),
    ['star', 'five-directions-legend'],
  )
})

test('stored room module lists do not infer newly released Echo', () => {
  const stored = ['star', 'hero-schools', 'five-directions-legend', 'spirit']
  assert.equal(normalizeRuleModules(stored).includes('echo'), false)
  assert.equal(normalizeServerRuleModules(stored).includes('echo'), false)
})

test('Tribulation is default-on, depends on Advanced Rules, and is not inferred for stored rooms', () => {
  const spec = RULE_MODULE_SPECS.find(module => module.id === 'tribulation')
  assert.deepEqual(spec, {
    id: 'tribulation',
    label: '主題規則‧天劫',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  })
  assert.equal(normalizeRuleModules(undefined).includes('tribulation'), true)
  assert.equal(normalizeRuleModules(['tribulation']).includes('tribulation'), false)
  assert.deepEqual(enableRuleModule([], 'tribulation'), [
    'star',
    'hero-schools',
    'five-directions-legend',
    'tribulation',
  ])
  const stored = ['star', 'hero-schools', 'five-directions-legend', 'echo']
  assert.equal(normalizeRuleModules(stored).includes('tribulation'), false)
  assert.equal(normalizeServerRuleModules(stored).includes('tribulation'), false)
})

test('generic dependency operations add requirements and remove dependents', () => {
  assert.deepEqual(enableRuleModule([], 'spirit'), [
    'star',
    'hero-schools',
    'five-directions-legend',
    'spirit',
  ])
  assert.deepEqual(
    disableRuleModule(
      ['star', 'hero-schools', 'five-directions-legend', 'spirit'],
      'hero-schools',
    ),
    ['star', 'five-directions-legend'],
  )
})
