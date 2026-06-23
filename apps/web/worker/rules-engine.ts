import type { LocalGameResponse } from '../app/types/fewfc'
import rulesModule from './wasm/fewfc.wasm'

interface FewfcWasmExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(len: number): number
  fewfc_dealloc(ptr: number, len: number): void
  fewfc_handle_request(ptr: number, len: number): bigint
}

let wasmInstance: WebAssembly.Instance | undefined

async function instance(): Promise<FewfcWasmExports> {
  wasmInstance ??= await WebAssembly.instantiate(rulesModule, {})

  return wasmInstance.exports as FewfcWasmExports
}

export async function callRulesEngine(request: unknown): Promise<LocalGameResponse> {
  const wasm = await instance()
  const input = new TextEncoder().encode(JSON.stringify(request))
  const inputPtr = wasm.fewfc_alloc(input.length)

  new Uint8Array(wasm.memory.buffer).set(input, inputPtr)

  const packed = wasm.fewfc_handle_request(inputPtr, input.length)
  wasm.fewfc_dealloc(inputPtr, input.length)

  const outputPtr = Number(packed >> 32n)
  const outputLen = Number(packed & 0xffffffffn)
  const outputBytes = new Uint8Array(wasm.memory.buffer, outputPtr, outputLen)
  const output = new TextDecoder().decode(outputBytes)
  wasm.fewfc_dealloc(outputPtr, outputLen)

  const parsed = JSON.parse(output) as LocalGameResponse | { error: string }

  if ('error' in parsed) {
    throw new Error(parsed.error)
  }

  return parsed
}
