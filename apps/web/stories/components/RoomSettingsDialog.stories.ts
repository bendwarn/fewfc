import type { Meta, StoryObj } from '@storybook/vue3-vite'
import RoomSettingsDialog from '~/components/RoomSettingsDialog.vue'

const meta = {
  title: '房間/建立房間設定',
  component: RoomSettingsDialog,
  tags: ['autodocs'],
  args: {
    ruleVersion: '5.17',
    name: '週末友誼賽',
    mode: 'duel',
    access: 'private',
    busy: false,
  },
} satisfies Meta<typeof RoomSettingsDialog>

export default meta
type Story = StoryObj<typeof meta>

export const PrivateDuel: Story = {}
export const PublicTeamRoom: Story = { args: { mode: 'team', access: 'public' } }
export const Submitting: Story = { args: { busy: true } }
