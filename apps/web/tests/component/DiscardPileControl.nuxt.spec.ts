import { mountSuspended } from '@nuxt/test-utils/runtime'
import { expect, it } from 'vitest'
import DiscardPileControl from '~/components/DiscardPileControl.vue'

it('announces an empty discard pile and never requests its composition dialog', async () => {
  const wrapper = await mountSuspended(DiscardPileControl, {
    props: {
      owner: null,
      count: 0,
      unavailable: true,
      open: false,
    },
  })

  const trigger = wrapper.get('button')
  expect(trigger.attributes('aria-disabled')).toBe('true')
  expect(trigger.attributes('aria-expanded')).toBe('false')
  expect(trigger.attributes('aria-label')).toBe('查看棄牌內容，共 0 張')
  await trigger.trigger('click')
  expect(wrapper.emitted('toggle')).toBeUndefined()
})
