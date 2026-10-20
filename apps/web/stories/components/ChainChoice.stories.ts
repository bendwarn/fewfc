import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { defineComponent, type Component } from 'vue'
import ChainChoice from '~/components/ChainChoice.vue'
import { exampleCards } from '../fixtures/components'

function battleChoiceDecorator(story: Component): Component {
  return defineComponent({
    components: { story },
    template: '<div style="min-height:100dvh; display:grid; place-items:center; padding:24px; background:radial-gradient(ellipse at 85% 100%, rgba(87,132,100,.28), transparent 56%), linear-gradient(180deg,#101b17,#09100d)"><section style="width:min(620px,calc(100vw - 48px)); max-height:calc(100dvh - 48px); overflow:auto; padding:24px; border:1px solid rgba(245,253,247,.28); border-radius:20px; background:linear-gradient(140deg,rgba(239,255,246,.18),rgba(122,174,143,.035) 45%,transparent 76%),rgba(20,40,32,.88); box-shadow:inset 0 1px rgba(255,255,255,.26),0 24px 60px rgba(0,0,0,.4); backdrop-filter:blur(22px) saturate(1.3)"><story /></section></div>',
  })
}

const meta = {
  title: '對局/連鎖選擇',
  component: ChainChoice,
  tags: ['autodocs'],
  parameters: { layout: 'fullscreen' },
  decorators: [battleChoiceDecorator],
  args: {
    pouchOwners: ['alice', 'bob'],
    pouchOwner: 'alice',
    pouchCards: exampleCards,
    pouchCard: exampleCards[0]!,
    triggerCards: exampleCards.slice(1),
    triggerCard: null,
    strategyOptions: [],
    selectedStrategy: null,
    selectedStrategyAction: null,
    selectedTarget: null,
    selectedStar: null,
    breakStar: false,
    selectedRetreatCard: null,
    handCards: exampleCards,
    playerLabel: player => player === 'alice' ? '小明' : '小華',
    strategyLabel: strategy => strategy,
    starLabel: star => star,
  },
} satisfies Meta<typeof ChainChoice>

export default meta
type Story = StoryObj<typeof meta>

export const ChoosePouchAndTrigger: Story = {}
