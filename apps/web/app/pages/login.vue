<template>
  <div class="auth-card">
    <div class="mobile-brand"><span class="brand-mark">五</span> 五行戰鬥牌</div>
    <h2>{{ authMode === 'sign-in' ? '登入對戰' : '建立帳號' }}</h2>
    <p class="muted">{{ authMode === 'sign-in' ? '使用 Email 登入，繼續你的對戰紀錄。' : '建立可在不同裝置使用的玩家身份。' }}</p>

    <form @submit.prevent="login">
      <template v-if="authMode === 'sign-up'">
        <label for="player-name">玩家名稱</label>
        <div class="input-wrap"><span aria-hidden="true">人</span><input id="player-name" v-model.trim="nameInput" type="text" maxlength="16" autocomplete="nickname" placeholder="顯示名稱"></div>
      </template>
      <label for="player-email">Email</label>
      <div class="input-wrap"><span aria-hidden="true">@</span><input id="player-email" v-model.trim="emailInput" type="email" autocomplete="email" placeholder="you@example.com" autofocus></div>
      <label for="player-password">密碼</label>
      <div class="input-wrap"><span aria-hidden="true">密</span><input id="player-password" v-model="passwordInput" type="password" :autocomplete="authMode === 'sign-in' ? 'current-password' : 'new-password'" placeholder="至少 10 個字元"></div>
      <p v-if="loginError" class="form-error" role="alert">{{ loginError }}</p>
      <button class="primary-button login-button" type="submit" :disabled="authBusy">{{ authBusy ? '處理中…' : authMode === 'sign-in' ? '登入' : '註冊並登入' }} <span aria-hidden="true">→</span></button>
    </form>

    <button class="auth-mode-button" type="button" @click="toggleAuthMode">{{ authMode === 'sign-in' ? '還沒有帳號？建立帳號' : '已經有帳號？返回登入' }}</button>
    <template v-if="enabledProviders.length">
      <div class="divider"><span>或使用社群帳號</span></div>
      <button v-for="provider in enabledProviders" :key="provider" class="ghost-button social-button" type="button" :disabled="authBusy" @click="signInSocial(provider)">
        使用 {{ providerLabel(provider) }} 繼續
      </button>
    </template>
    <div class="divider"><span>或使用訪客身份</span></div>
    <button class="ghost-button" type="button" :disabled="authBusy" @click="guestLogin">以訪客身份遊玩</button>
    <p class="terms">繼續即表示你同意遊戲規範與使用條款。</p>
  </div>
</template>

<script setup lang="ts">
import { authClient } from '~/lib/auth-client'
import { presentApiError } from '~/lib/api-error-presentation'
import { safeInternalPath } from '~/lib/navigation'
import { socialAuthFailureMessage } from '~/lib/social-auth-presentation'

definePageMeta({ layout: 'auth' })

const route = useRoute()
const router = useRouter()
const session = usePlayerSession()
const authMode = ref<'sign-in' | 'sign-up'>('sign-in')
const nameInput = ref('')
const emailInput = ref('')
const passwordInput = ref('')
const authBusy = ref(false)
const loginError = ref('')
const providers = ref({ google: false, github: false })
type SocialProvider = 'google' | 'github'
const enabledProviders = computed(() => (['google', 'github'] as const).filter(provider => providers.value[provider]))

function loginRedirect(): string {
  return safeInternalPath(route.query.redirect) ?? '/rooms'
}

async function login() {
  if (!emailInput.value || !passwordInput.value) {
    loginError.value = '請輸入 Email 與密碼'
    return
  }
  if (authMode.value === 'sign-up' && !nameInput.value) {
    loginError.value = '請輸入玩家名稱'
    return
  }

  authBusy.value = true
  loginError.value = ''
  try {
    const result = authMode.value === 'sign-in'
      ? await authClient.signIn.email({ email: emailInput.value, password: passwordInput.value, rememberMe: true })
      : await authClient.signUp.email({ name: nameInput.value, email: emailInput.value, password: passwordInput.value })
    if (result.error) {
      loginError.value = presentApiError({ data: { message: result.error.message } }, '無法完成登入')
      return
    }
    await session.refresh()
    await router.replace(loginRedirect())
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function guestLogin() {
  authBusy.value = true
  loginError.value = ''
  try {
    const result = await authClient.signIn.anonymous()
    if (result.error) {
      loginError.value = presentApiError({ data: { message: result.error.message } }, '無法建立訪客身份')
      return
    }
    await session.refresh()
    await router.replace(loginRedirect())
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function signInSocial(provider: SocialProvider) {
  authBusy.value = true
  loginError.value = ''
  try {
    const result = await authClient.signIn.social({
      provider,
      callbackURL: loginRedirect(),
      errorCallbackURL: '/login',
    })
    if (result.error) loginError.value = presentApiError({ data: { message: result.error.message } }, '無法開始社群登入')
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

function providerLabel(provider: SocialProvider) {
  return provider === 'google' ? 'Google' : 'GitHub'
}

function toggleAuthMode() {
  authMode.value = authMode.value === 'sign-in' ? 'sign-up' : 'sign-in'
  loginError.value = ''
}

onMounted(async () => {
  loginError.value = socialAuthFailureMessage(route.query.error) ?? ''
  try {
    providers.value = (await $fetch<{ providers: typeof providers.value }>('/api/auth-capabilities')).providers
  } catch {
    providers.value = { google: false, github: false }
  }
})
</script>
