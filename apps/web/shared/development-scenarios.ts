export type DevelopmentScenario =
  | { name: 'star-endgame' }
  | { name: 'hero-schools-transition' }
  | { name: 'spirit-skill'; options?: { spirit?: 'Metal' | 'Fire' } }
  | { name: 'echo-pure-fire'; options?: { mode?: 'actionDetail' } }
  | { name: 'tribulation-earth-rending' }

export const DEVELOPMENT_SCENARIO_NAMES: DevelopmentScenario['name'][] = [
  'star-endgame',
  'hero-schools-transition',
  'spirit-skill',
  'echo-pure-fire',
  'tribulation-earth-rending',
]

export function isDevelopmentScenario(value: unknown): value is DevelopmentScenario {
  if (!value || typeof value !== 'object' || !('name' in value)) return false
  const scenario = value as Record<string, unknown>
  if (!DEVELOPMENT_SCENARIO_NAMES.includes(
    String(scenario.name) as DevelopmentScenario['name'],
  )) return false

  const keys = Object.keys(scenario)
  if (scenario.name === 'star-endgame'
    || scenario.name === 'hero-schools-transition'
    || scenario.name === 'tribulation-earth-rending') {
    return keys.length === 1
  }
  if (keys.some(key => key !== 'name' && key !== 'options')) return false
  if (scenario.options === undefined) return true
  if (!scenario.options || typeof scenario.options !== 'object') return false
  const options = scenario.options as Record<string, unknown>
  if (scenario.name === 'spirit-skill') {
    return Object.keys(options).every(key => key === 'spirit')
      && (options.spirit === undefined || options.spirit === 'Metal' || options.spirit === 'Fire')
  }
  return Object.keys(options).every(key => key === 'mode')
    && (options.mode === undefined || options.mode === 'actionDetail')
}
