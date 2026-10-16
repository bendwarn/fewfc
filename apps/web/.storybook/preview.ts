import '../app/assets/css/main.css'
import { setup, type Preview } from '@storybook/vue3-vite'
import { defineComponent, h } from 'vue'

setup(app => {
  app.component('NuxtLink', defineComponent({
    props: { to: { type: [String, Object], default: '/' } },
    setup(props, { slots }) {
      return () => h('a', { href: typeof props.to === 'string' ? props.to : '/' }, slots.default?.())
    },
  }))
})

const preview: Preview = {
  parameters: {
    backgrounds: { default: '遊戲介面' },
    layout: 'padded',
  },
  beforeEach: () => {
    document.documentElement.dataset.theme = 'light'
  },
  decorators: [story => ({
    components: { story },
    template: '<main style="min-height: 100vh; background: var(--app-canvas); color: var(--app-text)"><story /></main>',
  })],
}

export default preview
