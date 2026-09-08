import { mountSuspended } from '@nuxt/test-utils/runtime'
import { defineComponent, nextTick, ref } from 'vue'
import { expect, it } from 'vitest'
import RoomSettingsDialog from '~/components/RoomSettingsDialog.vue'

it('focuses the room name, closes on Escape, and restores the trigger focus', async () => {
  const Host = defineComponent({
    components: { RoomSettingsDialog },
    setup() {
      const open = ref(true)
      const trigger = ref<HTMLButtonElement | null>(null)
      const close = () => {
        open.value = false
        void nextTick(() => trigger.value?.focus())
      }
      return { close, open, trigger }
    },
    template: `
      <button ref="trigger" type="button">建立房間</button>
      <RoomSettingsDialog
        v-if="open"
        name="測試房間"
        mode="duel"
        access="public"
        :busy="false"
        @close="close"
      />
    `,
  })

  const wrapper = await mountSuspended(Host, { attachTo: document.body })
  await nextTick()
  expect(document.querySelector('#room-name')).toBe(document.activeElement)

  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
  await nextTick()
  expect(document.querySelector('[role="dialog"]')).toBeNull()
  expect(wrapper.get('button').element).toBe(document.activeElement)
})

it('defaults the creation selector to 5.17 and emits a selected past version', async () => {
  const wrapper = await mountSuspended(RoomSettingsDialog, {
    props: { name: '版本測試', mode: 'duel', access: 'private', busy: false },
    global: { stubs: { teleport: true } },
  })
  expect(wrapper.text()).toContain('規則版本 5.17')
  expect(wrapper.get('details').element.open).toBe(false)
  await wrapper.get('summary').trigger('click')
  await wrapper.findAll('input[name="create-rule-version"]')[1]!.setValue(true)
  expect(wrapper.emitted('update:ruleVersion')).toEqual([['5.16']])
})
