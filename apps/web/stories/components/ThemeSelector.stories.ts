import type { Meta, StoryObj } from '@storybook/vue3-vite'
import ThemeSelector from '~/components/ThemeSelector.vue'

const meta = {
  title: '共用/外觀選擇器',
  component: ThemeSelector,
  tags: ['autodocs'],
} satisfies Meta<typeof ThemeSelector>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
