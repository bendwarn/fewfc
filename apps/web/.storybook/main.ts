import { fileURLToPath } from 'node:url'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import type { StorybookConfig } from '@storybook/vue3-vite'
import { pageImports } from './page-imports.ts'

const config: StorybookConfig = {
  stories: ['../stories/**/*.stories.@(ts|tsx)'],
  addons: ['@storybook/addon-mcp'],
  framework: { name: '@storybook/vue3-vite', options: {} },
  docs: { autodocs: 'tag' },
  features: { componentsManifest: true },
  staticDirs: ['../public'],
  async viteFinal(config) {
    config.resolve ??= {}
    config.resolve.alias = {
      ...config.resolve.alias,
      '@page-runtime': fileURLToPath(new URL('./mocks/page-runtime.ts', import.meta.url)),
      '@room-runtime': fileURLToPath(new URL('./mocks/room-runtime.ts', import.meta.url)),
      '~/lib/auth-client': fileURLToPath(new URL('./mocks/auth-client.ts', import.meta.url)),
      '~/composables/useColorTheme': fileURLToPath(new URL('./mocks/useColorTheme.ts', import.meta.url)),
      '~/lib/rules-catalog': fileURLToPath(new URL('./mocks/useRulesCatalog.ts', import.meta.url)),
      '~': fileURLToPath(new URL('../app', import.meta.url)),
      '@': fileURLToPath(new URL('../app', import.meta.url)),
      '#shared': fileURLToPath(new URL('../shared', import.meta.url)),
      '#shared/utils/rule-modules': fileURLToPath(new URL('../shared/utils/rule-modules.ts', import.meta.url)),
      '#shared/utils/rule-versions': fileURLToPath(new URL('../shared/utils/rule-versions.ts', import.meta.url)),
    }
    config.plugins ??= []
    config.plugins.unshift(pageImports(), vue())
    config.plugins.push(tailwindcss())
    return config
  },
}

export default config
