import { createError } from 'h3'
import type { PersonalDeckResolution, RulesCatalog } from '../../app/types/fewfc'
import type { PlayerDeckList } from '../../shared/game-room'

export interface FewfcRulesEngineBridge {
  catalog(): Promise<RulesCatalog>
  resolvePersonalDeck(player: string, candidate?: PlayerDeckList): Promise<PersonalDeckResolution>
  resolveRuleModules(candidate?: string[], ruleVersion?: '5.16' | '5.17'): Promise<{ modules: string[] }>
}

declare global {
  // 外層 Cloudflare Worker 會在分派到 Nitro 前安裝 WASM 橋接器。
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
