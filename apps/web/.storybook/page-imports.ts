import type { Plugin } from 'vite'

// Storybook 直接呈現生產頁面；只補上 Nuxt 的自動匯入，服務依賴由 fixture 提供。
export function pageImports(): Plugin {
  return {
    name: 'storybook-page-imports',
    enforce: 'pre',
    transform(source, id) {
      if (id.endsWith('/app/composables/useGameRoom.ts')) {
        return `import { ref, computed, watch, onBeforeUnmount, type Ref } from 'vue'\nimport { useRealtimeSocket } from '@room-runtime'\n${source.replaceAll('import.meta.client', 'true')}`
      }
      if (!id.endsWith('.vue') || !id.includes('/app/')) return
      const isRoom = id.endsWith('/app/pages/rooms/[gameId].vue')
      const isPage = (id.endsWith('/app/error.vue') || id.includes('/app/pages/')) && (!id.includes('/pages/rooms/') || isRoom) && !id.includes('/pages/__test/')
      const imports: string[] = []
      if (isPage) {
        const vueNames = ['ref', 'shallowRef', 'computed', 'reactive', 'watch', 'onMounted', 'onBeforeUnmount', 'nextTick']
          .filter(name => new RegExp(`\\b${name}(?:<|\\()`).test(source))
        if (vueNames.length) imports.push(`import { ${vueNames.join(', ')} } from 'vue'`)
        const nuxtNames = ['useRoute', 'useRouter', 'usePlayerSession', 'definePageMeta', 'clearError']
          .filter(name => source.includes(`${name}(`))
        if (nuxtNames.length) imports.push(`import { ${nuxtNames.join(', ')} } from '@page-runtime'`)
        if (isRoom) imports.push("import { useGameRoom } from '~/composables/useGameRoom'")
        if (isRoom) {
          for (const name of ['AnchoredSurface', 'GameCard', 'ChainChoice', 'CardChoiceMatrix', 'RuleModuleSettings', 'WaitingRoomLayout', 'CompetitionClockDisplay', 'CompetitionReadyControls', 'SplitEarthFormationChoice']) {
            imports.push(`import ${name} from '~/components/${name}.vue'`)
          }
        }
      }
      if ((isPage || id.includes('/app/components/Competition')) && source.includes('$fetch')) {
        imports.push("import { $fetch } from '@page-runtime'")
      }
      if (!imports.length) return
      const prepared = isRoom ? source.replace("from '~/lib/player-notifications-context'", "from '@room-runtime'") : source
      return prepared.replace(/(<script setup[^>]*>)/, `$1\n${imports.join('\n')}\n`)
    },
  }
}
