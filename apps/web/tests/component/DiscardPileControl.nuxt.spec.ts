import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'
import DiscardPileControl from '~/components/DiscardPileControl.vue'

const featuredCard = {
  id: 18,
  label: '火 3 級',
  element: 'Fire' as const,
  level: 3,
  secretStrategies: [],
}

describe('DiscardPileControl', () => {
  it('shows the last completed Turn Draw discard as a non-selectable card face', async () => {
    const wrapper = await mountSuspended(DiscardPileControl, {
      props: {
        owner: null,
        count: 4,
        deckCount: 37,
        featuredCard,
        unavailable: false,
        open: true,
      },
    })

    const trigger = wrapper.get('button')
    expect(wrapper.get('.discard-pile-label').text()).toBe('共用棄牌')
    expect(wrapper.get('.featured-discard-card').element.tagName).toBe('SPAN')
    expect(wrapper.get('.featured-discard-card').attributes('role')).toBe('img')
    expect(trigger.attributes('aria-expanded')).toBe('true')
    expect(trigger.attributes('aria-label')).toBe('查看共用棄牌詳情。目前棄牌 4 張，牌庫 37 張。上回合棄牌：火 3 級')

    await trigger.trigger('click')
    expect(wrapper.emitted('toggle')).toHaveLength(1)
  })

  it('shows discard and deck counts when no featured card is available', async () => {
    const wrapper = await mountSuspended(DiscardPileControl, {
      props: {
        owner: 'p1',
        ownerLabel: '甲',
        count: 2,
        deckCount: 18,
        unavailable: false,
        open: false,
      },
    })

    expect(wrapper.get('.discard-pile-label').text()).toBe('甲的棄牌')
    expect(wrapper.get('.discard-counts').text()).toBe('2—18')
    expect(wrapper.find('.featured-discard-card').exists()).toBe(false)
    expect(wrapper.get('button').attributes('aria-label')).toBe('查看甲的棄牌詳情。目前棄牌 2 張，牌庫 18 張')
  })

  it('keeps unavailable piles disabled and never emits a toggle', async () => {
    const wrapper = await mountSuspended(DiscardPileControl, {
      props: {
        owner: null,
        count: 0,
        deckCount: 45,
        unavailable: true,
        open: false,
      },
    })

    const trigger = wrapper.get('button')
    expect(trigger.attributes('disabled')).toBeDefined()
    expect(trigger.attributes('aria-disabled')).toBe('true')
    expect(trigger.attributes('aria-expanded')).toBe('false')
    expect(trigger.attributes('aria-label')).toBe('查看共用棄牌詳情。目前棄牌 0 張，牌庫 45 張')
    await trigger.trigger('click')
    expect(wrapper.emitted('toggle')).toBeUndefined()
  })

  it('keeps a historical face previewable when the current pile is empty', async () => {
    const wrapper = await mountSuspended(DiscardPileControl, {
      props: {
        owner: null,
        count: 0,
        deckCount: 45,
        featuredCard,
        unavailable: true,
        open: false,
      },
    })

    const trigger = wrapper.get('button')
    expect(trigger.attributes('disabled')).toBeUndefined()
    expect(trigger.attributes('aria-disabled')).toBe('true')
    expect(wrapper.get('.featured-discard-card').exists()).toBe(true)
    await trigger.trigger('click')
    expect(wrapper.emitted('toggle')).toBeUndefined()
  })
})
