/** A duplicate-tab Pouch response is a successful canonical selection elsewhere. */
export function isInitialPouchAlreadyChosen(error: unknown): boolean {
  const data = (error as {
    data?: { code?: unknown, data?: { code?: unknown } }
  }).data
  return data?.code === 'InitialPouchAlreadyChosen'
    || data?.data?.code === 'InitialPouchAlreadyChosen'
}
