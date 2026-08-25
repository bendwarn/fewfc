import { expect, test } from 'bun:test'
import { scrollBattleRecordToLatest } from './battle-record-scroll'

test('battle record scroll helper scrolls the overflow container to its latest entry', () => {
  const feed = { scrollHeight: 384, scrollTop: 12 }
  scrollBattleRecordToLatest(feed)
  expect(feed.scrollTop).toBe(384)
})
