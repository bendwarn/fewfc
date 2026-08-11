import { expect, test } from 'bun:test'
import { isInitialPouchAlreadyChosen } from './initial-pouch-selection'

test('duplicate Initial Pouch Selection from another tab reconciles instead of alarming', () => {
  expect(isInitialPouchAlreadyChosen({
    data: { code: 'InitialPouchAlreadyChosen' },
  })).toBe(true)
  expect(isInitialPouchAlreadyChosen({
    data: { data: { code: 'InitialPouchAlreadyChosen' } },
  })).toBe(true)
  expect(isInitialPouchAlreadyChosen({ data: { code: 'rulesValidation' } })).toBe(false)
})
