import { expect, test } from 'bun:test'
import { battleRecordContentRevision, scrollBattleRecordToLatest } from './battle-record-scroll'

test('battle record scroll helper scrolls the overflow container to its latest entry', () => {
  const feed = { scrollHeight: 384, scrollTop: 12 }
  scrollBattleRecordToLatest(feed)
  expect(feed.scrollTop).toBe(384)
})

test('battle record content revision changes when a turn group appears before its first entry', () => {
  expect(battleRecordContentRevision([])).not.toBe(battleRecordContentRevision([{ entries: [] }]))
})

test('battle record content revision changes when randomness enriches an existing entry', () => {
  const before = battleRecordContentRevision([{
    entries: [{ id: 'decision-1', title: '施展「淨火」', summary: '使用火 2、水 1。' }],
  }])
  const after = battleRecordContentRevision([{
    entries: [{ id: 'decision-1', title: '施展「淨火」', summary: '使用火 2、水 1。對方生命值減少 3 點。' }],
  }])

  expect(after).not.toBe(before)
})
