<template>
  <main class="account-page">
    <section class="setup-card account-card" aria-labelledby="account-heading">
      <div class="card-heading">
        <span class="step-number" aria-hidden="true">帳</span>
        <div>
          <h1 id="account-heading">帳號設定</h1>
          <p>管理登入方式。社群帳號只用於登入，不會變更遊戲內名稱。</p>
        </div>
      </div>

      <p v-if="message" class="account-message" role="status">{{ message }}</p>
      <p v-if="errorMessage" class="form-error" role="alert">{{ errorMessage }}</p>

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
        <p v-if="state.isAnonymous && !state.canUpgrade" class="form-error" role="alert">{{ state.upgradeMessage }}</p>
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
import { socialAuthFailureMessage } from '~/lib/social-auth-presentation'

type SocialProvider = 'google' | 'github'
interface AccountMethod {
  id: string
  providerId: string
  createdAt: string
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
    if (result.error) errorMessage.value = result.error.message || '無法開始連結社群帳號。'
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
    const statusMessage = (error as { data?: { statusMessage?: unknown } }).data?.statusMessage
    errorMessage.value = typeof statusMessage === 'string' ? statusMessage : '無法解除登入方式。'
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

.account-page { @apply mx-auto w-full max-w-2xl p-5 md:p-10; }
.account-card { @apply grid gap-7; }
.account-card h2 { @apply mb-3 font-serif text-lg; }
.account-list { @apply grid gap-2 p-0; list-style: none; }
.account-list li { @apply flex items-center justify-between gap-3 border border-line px-4 py-3 text-sm; border-radius: 9px; }
.account-list .secondary-button { @apply min-h-9 px-3 text-xs; }
.account-actions { @apply flex flex-wrap gap-2; }
.account-message { @apply border px-3 py-2 text-sm; border-color: var(--app-accent); border-radius: 8px; color: var(--app-text); }
</style>
