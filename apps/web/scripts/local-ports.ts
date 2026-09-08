export function port(name: string, fallback: number): number {
  const value = process.env[name] ?? String(fallback)
  if (!/^\d+$/.test(value) || Number(value) < 1024 || Number(value) > 65535) {
    throw new Error(`${name} must be an integer between 1024 and 65535`)
  }
  return Number(value)
}
export const ports = {
  dev: port('FEWFC_DEV_PORT', 8787),
  devInspector: port('FEWFC_DEV_INSPECTOR_PORT', 9229),
  e2e: port('FEWFC_E2E_PORT', 8727),
  e2eInspector: port('FEWFC_E2E_INSPECTOR_PORT', 9230),
  component: port('FEWFC_COMPONENT_PORT', 8730),
}
if (new Set(Object.values(ports)).size !== Object.values(ports).length) {
  throw new Error('FEWFC ports must be distinct')
}
