/**
 * 回合標題、項目與既有項目的摘要更新都會改變可見紀錄；後者包含可信隨機
 * 補入既有決策的結果，因此不能只計算群組或項目數。
 */
export function battleRecordContentRevision(groups: ReadonlyArray<{
  title?: string
  entries: ReadonlyArray<{ id?: string, title?: string, summary?: string }>
}>): string {
  return JSON.stringify(groups.map(group => ({
    title: group.title ?? '',
    entries: group.entries.map(entry => ({
      id: entry.id ?? '',
      title: entry.title ?? '',
      summary: entry.summary ?? '',
    })),
  })))
}

export function scrollBattleRecordToLatest(feed: Pick<HTMLElement, 'scrollHeight' | 'scrollTop'> | null): void {
  if (feed) feed.scrollTop = feed.scrollHeight
}
