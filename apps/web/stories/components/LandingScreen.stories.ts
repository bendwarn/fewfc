import { useColorTheme } from '../../.storybook/mocks/useColorTheme'
import { onBeforeUnmount } from 'vue'
import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, waitFor, within } from 'storybook/test'
import LandingScreen from '~/components/LandingScreen.vue'

const meta = {
  title: '頁面/首頁',
  component: LandingScreen,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof LandingScreen>
export default meta
type Story = StoryObj<typeof meta>

export const Light: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('link', { name: /立即遊玩/ })).toHaveAttribute('href', '/login')
    await expect(canvas.getByRole('link', { name: '了解玩法' })).toHaveAttribute('href', '#how-it-works')
    await waitFor(() => expect(canvas.getByRole('heading', { level: 1 })).toBeVisible())
  },
}

export const Dark: Story = {
  decorators: [story => ({
    components: { story },
    setup() {
      useColorTheme().setPreference('dark')
      onBeforeUnmount(() => { useColorTheme().setPreference('light') })
    },
    template: '<story />',
  })],
}
