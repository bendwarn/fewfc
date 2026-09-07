import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  // test-utils 的 Vitest 4 備援匯入仍會被 Vite 解析；Vitest 5 改由 runtime 提供。
  resolve: {
    alias: { 'vitest/environments': 'vitest/runtime' },
  },
  test: {
    include: ['tests/component/**/*.nuxt.spec.ts'],
    environment: 'nuxt',
    environmentOptions: {
      nuxt: {
        domEnvironment: 'happy-dom',
      },
    },
  },
})
