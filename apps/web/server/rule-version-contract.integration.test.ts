import { expect, test } from 'bun:test'
import type { RulesCatalog } from '../app/types/fewfc'
import { presentationForRuleModule } from '../shared/utils/rule-modules'
import { modulesForVersion } from '../shared/utils/rule-versions'

interface VersionExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(length: number): number
  fewfc_dealloc(pointer: number, length: number): void
  fewfc_rules_catalog(): bigint
  fewfc_resolve_rule_modules(pointer: number, length: number): bigint
}

// 此整合測試使用正式建置的 WASM，確保前端版本篩選與 Rust 公開契約一致。
test('version-filtered UI catalog matches the built Rust version resolver', async () => {
  const bytes = await Bun.file(new URL('../worker/wasm/fewfc.wasm', import.meta.url)).arrayBuffer()
  const { instance } = await WebAssembly.instantiate(bytes, {})
  const wasm = instance.exports as VersionExports
  function output<T>(packed: bigint): T {
    const pointer = Number(packed >> 32n)
    const length = Number(packed & 0xffffffffn)
    const json = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, pointer, length))
    wasm.fewfc_dealloc(pointer, length)
    return JSON.parse(json) as T
  }
  const catalog = output<RulesCatalog>(wasm.fewfc_rules_catalog())
  for (const module of catalog.ruleModules) {
    expect(presentationForRuleModule(module.id).label).not.toBe('其他規則')
  }
  for (const ruleVersion of ['5.16', '5.17'] as const) {
    const input = new TextEncoder().encode(JSON.stringify({ ruleVersion }))
    const pointer = wasm.fewfc_alloc(input.length)
    new Uint8Array(wasm.memory.buffer).set(input, pointer)
    const packed = wasm.fewfc_resolve_rule_modules(pointer, input.length)
    wasm.fewfc_dealloc(pointer, input.length)
    const result = output<{ modules: string[] }>(packed)
    const expected = modulesForVersion(catalog.ruleModules, ruleVersion)
      .filter(module => module.defaultEnabled).map(module => module.id)
    expect(result.modules).toEqual(expected)
    expect(result.modules.includes('totem-formation')).toBe(ruleVersion === '5.17')
    expect(result.modules.includes('echo')).toBe(ruleVersion === '5.16')
    expect(result.modules.includes('tribulation')).toBe(ruleVersion === '5.16')
  }
})
