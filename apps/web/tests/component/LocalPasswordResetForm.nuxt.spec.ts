import { mountSuspended } from '@nuxt/test-utils/runtime'
import { expect, it, vi } from 'vitest'
import LocalPasswordResetForm from '~/components/LocalPasswordResetForm.vue'

it('reports an unknown local account after a valid reset submission', async () => {
  const resetPassword = vi.fn().mockResolvedValue('user-not-found')
  const wrapper = await mountSuspended(LocalPasswordResetForm, {
    props: {
      resetPassword,
      completeReset: vi.fn(),
    },
  })

  await wrapper.get('#reset-password-email').setValue('missing@example.com')
  await wrapper.get('#reset-password-new').setValue('ReplacementPassword123!')
  await wrapper.get('#reset-password-confirm').setValue('ReplacementPassword123!')
  await wrapper.get('form').trigger('submit')

  expect(resetPassword).toHaveBeenCalledWith({
    email: 'missing@example.com',
    newPassword: 'ReplacementPassword123!',
    confirmPassword: 'ReplacementPassword123!',
  })
  expect(wrapper.get('[role="alert"]').text()).toBe('找不到這個 Email 的帳號')
})
