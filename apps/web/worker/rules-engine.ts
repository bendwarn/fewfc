import type {
  PersonalDeckResolution,
  RulesCatalog,
} from '../app/types/fewfc'
import type { CompetitionRequest } from '../shared/competition'
import type { PlayerDeckList, RulesEngineResult } from '../shared/game-room'
import { rulesEngineError } from './rules-engine-error'
import rulesModule from './wasm/fewfc.wasm'

interface FewfcWasmExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(len: number): number
  fewfc_dealloc(ptr: number, len: number): void
  fewfc_handle_request(ptr: number, len: number): bigint
  fewfc_handle_competition_request(ptr: number, len: number): bigint
  fewfc_rules_catalog(): bigint
  fewfc_resolve_personal_deck(ptr: number, len: number): bigint
  fewfc_resolve_rule_modules(ptr: number, len: number): bigint
}

let wasmInstance: WebAssembly.Instance | undefined

async function instance(): Promise<FewfcWasmExports> {
  if (!wasmInstance) {
    wasmInstance = await WebAssembly.instantiate(rulesModule, {})
  }

  return wasmInstance.exports as FewfcWasmExports
}

export async function callRulesEngine(request: unknown): Promise<RulesEngineResult> {
  const wasm = await instance()
  return callJsonExport(wasm, request, wasm.fewfc_handle_request)
}

/**
 * 低階 server-only competition ABI。呼叫者必須再經 competition-bridge 的
 * response parser 與 public projection，不能把此結果直接送到 HTTP。
 */
export async function callCompetitionEngineWasm(request: CompetitionRequest): Promise<unknown> {
  const wasm = await instance()
  if (!wasm.fewfc_handle_competition_request) {
    throw new Error('competition WASM export fewfc_handle_competition_request is unavailable')
  }
  return callJsonExport(
    wasm,
    request,
    wasm.fewfc_handle_competition_request,
    { preserveErrorEnvelope: true },
  )
}

export async function callPersonalDeckResolution(
  player: string,
  candidate?: PlayerDeckList,
): Promise<PersonalDeckResolution> {
  const wasm = await instance()
  return callJsonExport(
    wasm,
    { player, candidate },
    wasm.fewfc_resolve_personal_deck,
  )
}

export async function callRuleModuleResolution(
  candidate?: string[],
  ruleVersion: '5.16' | '5.17' = '5.17',
): Promise<{ modules: string[] }> {
  const wasm = await instance()
  return callJsonExport(wasm, { candidate, ruleVersion }, wasm.fewfc_resolve_rule_modules)
}

function callJsonExport<T>(
  wasm: FewfcWasmExports,
  request: unknown,
  invoke: (ptr: number, len: number) => bigint,
  options: { preserveErrorEnvelope?: boolean } = {},
): T {
  const input = new TextEncoder().encode(JSON.stringify(request))
  const inputPtr = wasm.fewfc_alloc(input.length)

  new Uint8Array(wasm.memory.buffer).set(input, inputPtr)

  let packed: bigint
  try {
    packed = invoke(inputPtr, input.length)
  } finally {
    wasm.fewfc_dealloc(inputPtr, input.length)
  }

  return readPackedJson<T>(wasm, packed, options)
}

export async function callRulesCatalog(): Promise<RulesCatalog> {
  const wasm = await instance()
  return readPackedJson<RulesCatalog>(wasm, wasm.fewfc_rules_catalog())
}

function readPackedJson<T>(
  wasm: FewfcWasmExports,
  packed: bigint,
  options: { preserveErrorEnvelope?: boolean } = {},
): T {
  const outputPtr = Number(packed >> 32n)
  const outputLen = Number(packed & 0xffffffffn)
  const outputBytes = new Uint8Array(wasm.memory.buffer, outputPtr, outputLen)
  const output = new TextDecoder().decode(outputBytes)
  wasm.fewfc_dealloc(outputPtr, outputLen)

  const parsed = JSON.parse(output) as unknown

  if (!options.preserveErrorEnvelope && parsed && typeof parsed === 'object' && 'error' in parsed) {
    throw rulesEngineError(parsed.error)
  }

  return parsed as T
}
