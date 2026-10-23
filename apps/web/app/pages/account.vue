<template>
  <main class="account-page workspace-page page-scene" data-scene="inn">
    <PageIntro title="帳號設定" eyebrow="玩家身份" description="管理登入方式。社群帳號只用於登入，不會變更遊戲內名稱。" back-to="/rooms" back-label="房間大廳" />
    <section class="setup-card account-card" aria-label="管理登入方式">
      <p v-if="message" class="page-notice page-notice--success" role="status">{{ message }}</p>
      <p v-if="errorMessage" class="page-notice page-notice--error" role="alert">{{ errorMessage }}</p>

      <section aria-labelledby="linked-heading">
        <h2 id="linked-heading">已連結的登入方式</h2>
        <ul class="account-list">
          <li v-for="account in state.accounts" :key="account.id">
            <span>{{ providerLabel(account.providerId) }}</span>
            <button
              class="secondary-button"
              type="button"
              :disabled="busy || state.accounts.length <= 1"
              @click="unlink(account.id)"
            >解除連結</button>
          </li>
        </ul>
        <p v-if="state.accounts.length <= 1" class="muted">至少需要保留一種登入方式。</p>
      </section>

      <section aria-labelledby="link-heading">
        <h2 id="link-heading">新增登入方式</h2>
        <p v-if="state.isAnonymous && !state.canUpgrade" class="page-notice page-notice--error" role="alert">{{ state.upgradeMessage }}</p>
        <div class="account-actions">
          <button
            v-for="provider in enabledProviders"
            :key="provider"
            class="ghost-button"
            type="button"
            :disabled="busy || (state.isAnonymous && !state.canUpgrade) || isLinked(provider)"
            @click="link(provider)"
          >連結 {{ providerLabel(provider) }}</button>
        </div>
        <p v-if="!enabledProviders.length" class="muted">此環境尚未設定可用的社群登入方式。</p>
      </section>
    </section>
  </main>
</template>

<script setup lang="ts">
import { authClient } from '~/lib/auth-client'
import { presentApiError } from '~/lib/api-error-presentation'
import { socialAuthFailureMessage } from '~/lib/social-auth-presentation'

type SocialProvider = 'google' | 'github'
interface AccountMethod {
  id: string
  providerId: string
}

const busy = ref(false)
const message = ref('')
const errorMessage = ref('')
const route = useRoute()
const state = reactive({
  providers: { google: false, github: false },
  accounts: [] as AccountMethod[],
  isAnonymous: false,
  canUpgrade: true,
  upgradeMessage: '',
})
const enabledProviders = computed(() => (['google', 'github'] as const).filter(provider => state.providers[provider]))

async function refresh() {
  const response = await $fetch<{
    providers: typeof state.providers
    accounts: AccountMethod[]
    isAnonymous: boolean
    canUpgrade: boolean
    upgradeMessage?: string
  }>('/api/account/auth-methods')
  state.providers = response.providers
  state.accounts = response.accounts
  state.isAnonymous = response.isAnonymous
  state.canUpgrade = response.canUpgrade
  state.upgradeMessage = response.upgradeMessage ?? ''
}

function providerLabel(provider: string) {
  if (provider === 'credential') return 'Email 與密碼'
  if (provider === 'google') return 'Google'
  if (provider === 'github') return 'GitHub'
  return provider
}

function isLinked(provider: SocialProvider) {
  return state.accounts.some(account => account.providerId === provider)
}

async function link(provider: SocialProvider) {
  busy.value = true
  errorMessage.value = ''
  try {
    const result = state.isAnonymous
      ? await authClient.signIn.social({
          provider,
          callbackURL: '/account',
          errorCallbackURL: '/account',
        })
      : await authClient.linkSocial({
          provider,
          callbackURL: '/account',
          errorCallbackURL: '/account',
      })
    if (result.error) errorMessage.value = presentApiError({ data: { message: result.error.message } }, '無法開始連結社群帳號。')
  } catch {
    errorMessage.value = '帳號服務目前無法使用。'
  } finally {
    busy.value = false
  }
}

async function unlink(accountId: string) {
  busy.value = true
  message.value = ''
  errorMessage.value = ''
  try {
    await $fetch('/api/account/unlink', { method: 'POST', body: { accountId } })
    await refresh()
    message.value = '登入方式已解除連結。'
  } catch (error) {
    errorMessage.value = presentApiError(error, '無法解除登入方式。')
  } finally {
    busy.value = false
  }
}

onMounted(() => {
  errorMessage.value = socialAuthFailureMessage(route.query.error) ?? ''
  void refresh().catch(() => {
    errorMessage.value = '無法讀取帳號設定。'
  })
})
</script>

<style scoped>
@reference "../assets/css/main.css";

.account-page { max-width: 840px; }
.account-card { @apply grid gap-7; }
.account-list .secondary-button { @apply min-h-9 px-3 text-xs; }
.account-card section { display: grid; gap: 16px; }
.account-card section + section { border-top: 1px solid var(--app-border); padding-top: 24px; }
.account-card h2 { font: 700 20px / 1.5 var(--font-serif); }
.account-list { display: grid; gap: 12px; margin: 0; padding: 0; list-style: none; }
.account-list li { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 16px; border: 1px solid var(--ui-glass-stroke); border-radius: 16px; background: color-mix(in srgb,var(--app-surface) 80%,transparent); }
.account-list .secondary-button { min-height: 40px; font-size: 12px; }
.account-actions { display: flex; flex-wrap: wrap; gap: 12px; }
.account-card .muted { font-size: 13px; line-height: 1.8; color: var(--app-text-muted); }
</style>
