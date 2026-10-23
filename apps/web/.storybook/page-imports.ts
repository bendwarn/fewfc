import type { Plugin } from 'vite'

// Storybook 直接呈現生產頁面；只補上 Nuxt 的自動匯入，服務依賴由 fixture 提供。
export function pageImports(): Plugin {
  return {
    name: 'storybook-page-imports',
    enforce: 'pre',
    transform(source, id) {
      if (!id.endsWith('.vue') || !id.includes('/app/')) return
      const isPage = (id.endsWith('/app/error.vue') || id.includes('/app/pages/')) && !id.includes('/pages/rooms/') && !id.includes('/pages/__test/')
      const imports: string[] = []
      if (isPage) {
        const vueNames = ['ref', 'shallowRef', 'computed', 'reactive', 'watch', 'onMounted', 'onBeforeUnmount', 'nextTick']
          .filter(name => new RegExp(`\\b${name}(?:<|\\()`).test(source))
        if (vueNames.length) imports.push(`import { ${vueNames.join(', ')} } from 'vue'`)
        const nuxtNames = ['useRoute', 'useRouter', 'usePlayerSession', 'definePageMeta', 'clearError']
          .filter(name => source.includes(`${name}(`))
        if (nuxtNames.length) imports.push(`import { ${nuxtNames.join(', ')} } from '@page-runtime'`)
      }
      if ((isPage || id.includes('/app/components/Competition')) && source.includes('$fetch')) {
        imports.push("import { $fetch } from '@page-runtime'")
      }
      if (!imports.length) return
      return source.replace(/(<script setup[^>]*>)/, `$1\n${imports.join('\n')}\n`)
    },
  }
}
