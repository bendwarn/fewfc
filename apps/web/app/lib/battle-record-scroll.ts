/** 將真正可捲動的戰局紀錄容器移到最新一筆。 */
export function scrollBattleRecordToLatest(feed: Pick<HTMLElement, 'scrollHeight' | 'scrollTop'> | null): void {
  if (feed) feed.scrollTop = feed.scrollHeight
}
