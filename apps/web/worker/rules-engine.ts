import type {
  PersonalDeckResolution,
  RulesCatalog,
} from '../app/types/fewfc'
import type { PlayerDeckList, RulesEngineResult } from '../shared/game-room'
import { rulesEngineError } from './rules-engine-error'
import rulesModule from './wasm/fewfc.wasm'

interface FewfcWasmExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(len: number): number
  fewfc_dealloc(ptr: number, len: number): void
  fewfc_handle_request(ptr: number, len: number): bigint
  fewfc_rules_catalog(): bigint
  fewfc_resolve_personal_deck(ptr: number, len: number): bigint
  fewfc_resolve_rule_modules(ptr: number, len: number): bigint
}

let wasmInstance: WebAssembly.Instance | undefined

async function instance(): Promise<FewfcWasmExports> {
  if (!wasmInstance) {
    wasmInstance = await WebAssembly.instantiate(rulesModule, {}) as WebAssembly.Instance
  }

  return wasmInstance.exports as FewfcWasmExports
}

export async function callRulesEngine(request: unknown): Promise<RulesEngineResult> {
  const wasm = await instance()
  return callJsonExport(wasm, request, wasm.fewfc_handle_request)
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
): Promise<{ modules: string[] }> {
  const wasm = await instance()
  return callJsonExport(wasm, { candidate }, wasm.fewfc_resolve_rule_modules)
}

function callJsonExport<T>(
  wasm: FewfcWasmExports,
  request: unknown,
  invoke: (ptr: number, len: number) => bigint,
): T {
  const input = new TextEncoder().encode(JSON.stringify(request))
  const inputPtr = wasm.fewfc_alloc(input.length)

  new Uint8Array(wasm.memory.buffer).set(input, inputPtr)

  const packed = invoke(inputPtr, input.length)
  wasm.fewfc_dealloc(inputPtr, input.length)

  return readPackedJson<T>(wasm, packed)
}

export async function callRulesCatalog(): Promise<RulesCatalog> {
  const wasm = await instance()
  return readPackedJson<RulesCatalog>(wasm, wasm.fewfc_rules_catalog())
}

function readPackedJson<T>(wasm: FewfcWasmExports, packed: bigint): T {
  const outputPtr = Number(packed >> 32n)
  const outputLen = Number(packed & 0xffffffffn)
  const outputBytes = new Uint8Array(wasm.memory.buffer, outputPtr, outputLen)
  const output = new TextDecoder().decode(outputBytes)
  wasm.fewfc_dealloc(outputPtr, outputLen)

  const parsed = JSON.parse(output) as T | { error: unknown }

  if ('error' in parsed) {
    throw rulesEngineError(parsed.error)
  }

  return parsed as T
}
