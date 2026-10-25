import { useColorTheme } from '../../.storybook/mocks/useColorTheme'
import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { onBeforeUnmount } from 'vue'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import PlayerShell from '~/components/PlayerShell.vue'
import AuthLayout from '~/layouts/auth.vue'
import LoginPage from '~/pages/login.vue'
import DeckPage from '~/pages/deck.vue'
import AccountPage from '~/pages/account.vue'
import ReplaysPage from '~/pages/replays/index.vue'
import ReplayPage from '~/pages/replays/[replayId].vue'
import CompetitionPage from '~/pages/competitions/index.vue'
import CompetitionDetail from '~/pages/competitions/[id].vue'
import PrivacyPage from '~/pages/privacy.vue'
import ErrorPage from '~/error.vue'
import { setupPageFixture, type PageScenario } from '../fixtures/pages'
import { pageRequests, pageNavigations } from '../../.storybook/mocks/page-runtime'

const screens = { login: LoginPage, deck: DeckPage, account: AccountPage, replays: ReplaysPage, replay: ReplayPage, competition: CompetitionPage, detail: CompetitionDetail, privacy: PrivacyPage, missing: ErrorPage }
const meta = {
  title: '頁面/完整功能頁',
  component: PlayerShell,
  args: { displayName: '小明', connectionState: 'connected', notifications: [] },
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof PlayerShell>
export default meta
type Story = StoryObj<typeof meta>

function screen(name: keyof typeof screens, dark = false, scenario: PageScenario = 'normal'): Story {
  return {
    render: () => ({
      components: { Screen: screens[name], PlayerShell, AuthLayout },
      setup() {
        setupPageFixture(`/${name}`, scenario)
        useColorTheme().setPreference(dark ? 'dark' : 'light')
        onBeforeUnmount(() => { useColorTheme().setPreference('light') })
        return { name }
      },
      template: name === 'login' ? '<AuthLayout><Screen /></AuthLayout>'
        : name === 'privacy' ? '<Screen />'
          : name === 'missing' ? '<Screen :error="{ statusCode: 404 }" />'
            : '<PlayerShell display-name="小明" connection-state="connected" :notifications="[]"><Screen /></PlayerShell>',
    }),
  }
}

export const Login = screen('login')
export const LoginDark = screen('login', true)
export const Deck = screen('deck')
export const DeckDark = screen('deck', true)
export const DeckError = screen('deck', true, 'error')
export const Account = screen('account')
export const AccountDark = screen('account', true)
export const Replays = screen('replays')
export const ReplaysDark = screen('replays', true)
export const ReplaysEmpty = screen('replays', true, 'empty')
export const ReplaysError = screen('replays', false, 'error')
export const Replay = screen('replay')
export const ReplayDark = screen('replay', true)
export const ReplayFinished: Story = {
  ...screen('replay', true, 'finished'),
  tags: ['conclusion'],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await userEvent.click(await canvas.findByRole('button', { name: '最後一步' }))
    const result = await canvas.findByRole('region', { name: '對局結算' })
    await expect(within(result).getByRole('heading', { name: '我方 勝利' })).toBeVisible()
  },
}
export const Competition = screen('competition')
export const CompetitionDark = screen('competition', true)
export const CompetitionDetails = screen('detail')
export const CompetitionDetailsDark = screen('detail', true)
export const Privacy = screen('privacy')
export const PrivacyDark = screen('privacy', true)
export const NotFound: Story = {
  ...screen('missing'),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('heading', { name: '此處沒有找到頁面' })).toBeVisible()
    await userEvent.click(canvas.getByRole('button', { name: '返回首頁' }))
    await expect(pageNavigations).toContain('/')
  },
}
export const LoginError: Story = {
  ...screen('login', true),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await userEvent.type(canvas.getByRole('textbox', { name: 'Email' }), 'story@example.test')
    await userEvent.type(canvas.getByLabelText('密碼', { exact: true }), 'story-password')
    await userEvent.click(canvas.getByRole('button', { name: '登入', exact: true }))
    await waitFor(() => expect(canvas.getByRole('alert')).toBeVisible())
    await expect(canvas.getByRole('button', { name: '登入', exact: true })).toBeEnabled()
  },
}

export const DeleteReplay: Story = {
  ...screen('replays'),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await waitFor(() => expect(canvas.getByText('青木試陣')).toBeVisible())
    const trigger = canvas.getAllByRole('button', { name: '刪除', exact: true })[0]!
    await userEvent.click(trigger)
    const dialog = within(document.body).getByRole('dialog', { name: '刪除此重播？' })
    await waitFor(() => expect(within(dialog).getByRole('button', { name: '取消' })).toHaveFocus())
    await userEvent.keyboard('{Escape}')
    await waitFor(() => expect(trigger).toHaveFocus())
    await userEvent.click(trigger)
    await userEvent.click(within(document.body).getByRole('button', { name: '刪除重播', exact: true }))
    await waitFor(() => expect(pageRequests.some(request => request.path === '/api/replays/replay-0' && request.options.method === 'DELETE')).toBe(true))
    await waitFor(() => expect(canvas.getAllByRole('button', { name: '觀看' })).toHaveLength(1))
  },
}

export const DeckImport: Story = {
  ...screen('deck'),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const trigger = await canvas.findByRole('button', { name: '匯入牌組' })
    await userEvent.click(trigger)
    const dialog = within(document.body).getByRole('dialog', { name: '匯入牌組' })
    const input = within(dialog).getByRole('textbox', { name: '牌組張數' })
    await waitFor(() => expect(input).toHaveFocus())
    await userEvent.type(input, 'abc')
    await userEvent.click(within(dialog).getByRole('button', { name: '套用' }))
    await expect(within(dialog).getByRole('alert')).toBeVisible()
    await userEvent.keyboard('{Escape}')
    await waitFor(() => expect(trigger).toHaveFocus())
  },
}
