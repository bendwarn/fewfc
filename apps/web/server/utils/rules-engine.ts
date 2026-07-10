import { createError } from 'h3'
import type { PersonalDeckResolution, RulesCatalog } from '../../app/types/fewfc'
import type { PlayerDeckList } from '../../shared/game-room'

export interface FewfcRulesEngineBridge {
  catalog(): Promise<RulesCatalog>
  resolvePersonalDeck(player: string, candidate?: PlayerDeckList): Promise<PersonalDeckResolution>
  resolveRuleModules(candidate?: string[]): Promise<{ modules: string[] }>
}

declare global {
  // The outer Cloudflare Worker installs the WASM bridge before dispatching to Nitro.
  // eslint-disable-next-line no-var
  var __fewfcRulesEngine__: FewfcRulesEngineBridge | undefined
}

export function rulesEngine(): FewfcRulesEngineBridge {
  if (!globalThis.__fewfcRulesEngine__) {
    throw createError({
      statusCode: 501,
      statusMessage: 'Rules Engine bindings are unavailable. Run the app through Wrangler.',
    })
  }
  return globalThis.__fewfcRulesEngine__
}
