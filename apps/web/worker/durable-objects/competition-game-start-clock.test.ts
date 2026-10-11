import { expect, test } from 'bun:test'

const fixturePath = `${import.meta.dir}/competition-game-start-clock.fixture.ts`
const webRoot = new URL('../..', import.meta.url).pathname
const defaultWasmPath = new URL('../wasm/fewfc.wasm', import.meta.url).pathname

test('正式 GameRoom clock regression runs with isolated Rust/WASM and worker mocks', async () => {
  const child = Bun.spawn([process.execPath, 'test', fixturePath], {
    cwd: webRoot,
    env: {
      ...Bun.env,
      FEWC_COMPETITION_TEST_WASM: Bun.env.FEWC_COMPETITION_TEST_WASM ?? defaultWasmPath,
    },
    stdout: 'pipe',
    stderr: 'pipe',
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ])

  expect(exitCode, `${stdout}\n${stderr}`).toBe(0)
})
