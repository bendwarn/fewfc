import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  hasValidServerRuleModuleDependencies,
  normalizeServerRuleModules,
} from '../../server/utils/rule-modules'
import { normalizeRuleModules } from '../../shared/game-room'

test('server and room Rule Module allowlists stay aligned', () => {
  assert.deepEqual(normalizeServerRuleModules(undefined), normalizeRuleModules(undefined))
  assert.deepEqual(
    normalizeServerRuleModules(['star', 'five-directions-legend', 'unknown']),
    normalizeRuleModules(['star', 'five-directions-legend', 'unknown']),
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
