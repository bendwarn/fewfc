<template>
  <LocalPasswordResetForm
    :reset-password="resetPassword"
    :complete-reset="completeReset"
    @return-to-login="returnToLogin"
  />
</template>

<script setup lang="ts">
import type { LocalPasswordResetResult } from '#shared/local-password-reset'
import { authClient } from '~/lib/auth-client'
import { safeInternalPath } from '~/lib/navigation'

definePageMeta({ layout: 'auth' })

const route = useRoute()
const router = useRouter()
const session = usePlayerSession()
function loginRedirect(): string {
  return safeInternalPath(route.query.redirect) ?? '/rooms'
}

function returnToLogin() {
  const redirect = safeInternalPath(route.query.redirect)
  void router.push({ path: '/login', query: redirect ? { redirect } : {} })
}

async function resetPassword(input: {
  email: string
  newPassword: string
  confirmPassword: string
}): Promise<LocalPasswordResetResult> {
  const response = await $fetch<{ status: LocalPasswordResetResult }>('/api/local-password-reset', {
    method: 'POST',
    body: input,
  })
  return response.status
}

async function completeReset(input: { email: string, password: string }): Promise<string | undefined> {
  try {
    const result = await authClient.signIn.email({ email: input.email, password: input.password, rememberMe: true })
    if (result.error) {
      return result.error.message || '密碼已重設，但自動登入失敗'
    }
    await session.refresh()
    await router.replace(loginRedirect())
  } catch {
    return '本機重設目前無法使用'
  }
}
</script>
