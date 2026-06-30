import assert from 'node:assert/strict'
import { test } from 'node:test'
import { normalizeServerRuleModules } from '../../server/utils/rule-modules'
import { normalizeRuleModules } from '../../shared/game-room'

test('server and room Rule Module allowlists stay aligned', () => {
  assert.deepEqual(normalizeServerRuleModules(undefined), normalizeRuleModules(undefined))
  assert.deepEqual(
    normalizeServerRuleModules(['five-directions-legend', 'unknown']),
    normalizeRuleModules(['five-directions-legend', 'unknown']),
  )
})
