import assert from 'node:assert/strict'
import { test } from 'node:test'
import { normalizeServerRuleModules } from '../../server/utils/rule-modules'
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
