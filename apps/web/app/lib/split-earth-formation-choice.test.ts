import { expect, test } from 'bun:test'
import {
  splitEarthChoiceKey,
  splitEarthRuleLabel,
  usesSplitEarthFormationGroups,
} from '../../shared/utils/split-earth-formation-choice'

test('presents the base ruleset and Rule Module names for Split Earth', () => {
  expect(splitEarthRuleLabel(null)).toBe('基礎規則')
  expect(splitEarthRuleLabel('five-directions-legend')).toBe('五方傳說')
  expect(splitEarthRuleLabel('echo')).toBe('迴響')
})

test('uses grouped choices only for Split Earth and keys each Pending Choice', () => {
  const choice = {
    formationGroups: [{
      ruleModuleId: null,
      formations: [{ id: 'weapon', name: '武器' }],
    }],
  }

  expect(usesSplitEarthFormationGroups(choice)).toBe(true)
  expect(usesSplitEarthFormationGroups({
    ...choice,
    formationGroups: [],
  })).toBe(false)
  expect(splitEarthChoiceKey(choice, 3, true)).not.toBe(
    splitEarthChoiceKey(choice, 4, true),
  )
  expect(splitEarthChoiceKey(choice, 3, true)).not.toBe(
    splitEarthChoiceKey(choice, 3, false),
  )
  expect(splitEarthChoiceKey(choice, 3, true)).not.toBe(splitEarthChoiceKey({
    ...choice,
    formationGroups: [{
      ruleModuleId: 'base:weapon',
      formations: [],
    }],
  }, 3, true))
})
