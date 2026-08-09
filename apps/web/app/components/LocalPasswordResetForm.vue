<template>
  <div class="auth-card">
    <div class="mobile-brand"><span class="brand-mark">五</span> 五行戰鬥牌</div>
    <h2>重設本機密碼</h2>
    <p class="muted">此入口僅適用於已啟用的本機開發環境，會直接更新既有的 Email 密碼登入憑證。</p>
    <form @submit.prevent="submit">
      <label for="reset-password-email">Email</label>
      <div class="input-wrap"><span aria-hidden="true">@</span><input id="reset-password-email" v-model.trim="email" type="email" autocomplete="email" placeholder="you@example.com" autofocus></div>
      <label for="reset-password-new">新密碼</label>
      <div class="input-wrap"><span aria-hidden="true">密</span><input id="reset-password-new" v-model="password" type="password" autocomplete="new-password" minlength="10" maxlength="128" placeholder="至少 10 個字元"></div>
      <label for="reset-password-confirm">確認新密碼</label>
      <div class="input-wrap"><span aria-hidden="true">密</span><input id="reset-password-confirm" v-model="confirmation" type="password" autocomplete="new-password" minlength="10" maxlength="128" placeholder="再次輸入新密碼"></div>
      <p v-if="errorMessage" class="form-error" role="alert">{{ errorMessage }}</p>
      <button class="primary-button login-button" type="submit" :disabled="busy">{{ busy ? '重設中…' : '重設密碼並登入' }} <span aria-hidden="true">→</span></button>
    </form>
    <button class="auth-mode-button" type="button" :disabled="busy" @click="$emit('return-to-login')">返回登入</button>
  </div>
</template>

<script setup lang="ts">
import type { LocalPasswordResetResult } from '#shared/local-password-reset'

interface ResetPasswordInput {
  email: string
  newPassword: string
  confirmPassword: string
}

interface CompletedReset {
  email: string
  password: string
}

const props = defineProps<{
  resetPassword: (input: ResetPasswordInput) => Promise<LocalPasswordResetResult>
  completeReset: (input: CompletedReset) => Promise<string | undefined>
}>()

defineEmits<{
  'return-to-login': []
}>()

const email = ref('')
const password = ref('')
const confirmation = ref('')
const busy = ref(false)
const errorMessage = ref('')

function unexpectedResetError(error: unknown): string {
  const message = (error as { data?: { statusMessage?: unknown } }).data?.statusMessage
  return typeof message === 'string' ? message : '本機重設目前無法使用'
}

async function submit() {
  if (!email.value || !password.value || !confirmation.value) {
    errorMessage.value = '請輸入 Email、新密碼與確認密碼'
    return
  }
  if (password.value !== confirmation.value) {
    errorMessage.value = '兩次輸入的新密碼不一致'
    return
  }
  if (password.value.length < 10) {
    errorMessage.value = '密碼至少需要 10 個字元'
    return
  }

  busy.value = true
  errorMessage.value = ''
  try {
    const result = await props.resetPassword({
      email: email.value,
      newPassword: password.value,
      confirmPassword: confirmation.value,
    })
    if (result === 'user-not-found') {
      errorMessage.value = '找不到這個 Email 的帳號'
      return
    }
    if (result === 'no-credential') {
      errorMessage.value = '此帳號沒有可重設的密碼登入憑證，且不會建立新憑證'
      return
    }

    errorMessage.value = await props.completeReset({ email: email.value, password: password.value }) ?? ''
  } catch (error) {
    errorMessage.value = unexpectedResetError(error)
  } finally {
    busy.value = false
  }
}
</script>
