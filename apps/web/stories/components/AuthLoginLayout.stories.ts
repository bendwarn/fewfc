import type { Meta, StoryObj } from '@storybook/vue3-vite'
import AuthLayout from '~/layouts/auth.vue'

const meta = {
  title: '頁面/登入版面',
  component: AuthLayout,
  parameters: { layout: 'fullscreen' },
  render: () => ({
    components: { AuthLayout },
    setup() {
      return { providers: ['Google', 'GitHub'] }
    },
    template: `
      <AuthLayout>
        <div class="auth-card">
          <div class="mobile-brand">五行戰鬥牌</div>
          <h2>建立帳號</h2>
          <p class="muted">建立可在不同裝置使用的玩家身份。</p>
          <form>
            <label for="story-player-name">玩家名稱</label>
            <div class="input-wrap"><span aria-hidden="true">人</span><input id="story-player-name" type="text" placeholder="顯示名稱"></div>
            <label for="story-player-email">Email</label>
            <div class="input-wrap"><span aria-hidden="true">@</span><input id="story-player-email" type="email" placeholder="you@example.com"></div>
            <label for="story-player-password">密碼</label>
            <div class="input-wrap"><span aria-hidden="true">密</span><input id="story-player-password" type="password" placeholder="至少 10 個字元"></div>
            <button class="primary-button login-button" type="button">註冊並登入 <span aria-hidden="true">→</span></button>
          </form>
          <button class="auth-mode-button" type="button">已經有帳號？返回登入</button>
          <div class="divider"><span>或使用社群帳號</span></div>
          <button v-for="provider in providers" :key="provider" class="ghost-button social-button" type="button">使用 {{ provider }} 繼續</button>
          <div class="divider"><span>或使用訪客身份</span></div>
          <button class="ghost-button" type="button">以訪客身份遊玩</button>
          <p class="terms">繼續即表示你同意遊戲規範與使用條款。</p>
        </div>
      </AuthLayout>
    `,
  }),
} satisfies Meta<typeof AuthLayout>

export default meta
type Story = StoryObj<typeof meta>

export const LongMethods: Story = {}
