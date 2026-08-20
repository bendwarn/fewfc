/** 重複分頁的袋牌回應代表其他地方已成功完成標準選擇。 */
export function isInitialPouchAlreadyChosen(error: unknown): boolean {
  const data = (error as {
    data?: { code?: unknown, data?: { code?: unknown } }
  }).data
  return data?.code === 'InitialPouchAlreadyChosen'
    || data?.data?.code === 'InitialPouchAlreadyChosen'
}
