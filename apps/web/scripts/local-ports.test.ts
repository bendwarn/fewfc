import { expect, test } from 'bun:test'
import { port } from './local-ports'
test('port rejects malformed and privileged values', () => {
  for (const value of ['abc', '0', '80', '65536', '1234.5']) {
    process.env.FEWFC_TEST_PORT = value
    expect(() => port('FEWFC_TEST_PORT', 8787)).toThrow()
  }
  delete process.env.FEWFC_TEST_PORT
  expect(port('FEWFC_TEST_PORT', 8787)).toBe(8787)
})
