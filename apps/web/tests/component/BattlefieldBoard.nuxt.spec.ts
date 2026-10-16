import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import { emptyPublicState } from '#shared/game-room'

describe('BattlefieldBoard', () => {
  it.each(['live', 'replay'] as const)('shows player-owned Totems and their rules in %s', async (mode) => {
    const state = emptyPublicState(['alice', 'bob'])
    state.totems = [{ player: 'alice', totem: 'AzureHorn', name: '青角圖騰' }]
    const wrapper = await mountSuspended(BattlefieldBoard, {
      props: { state, displayNames: {}, anchorPlayer: 'alice', mode },
    })
    const totem = wrapper.get('[aria-label^="青角圖騰："]')
    expect(totem.text()).toBe('青角圖騰')
    expect(totem.attributes('title')).toContain('防護罩不適用')
    expect(totem.attributes('tabindex')).toBe('0')
    await wrapper.setProps({ state: { ...state, totems: [] } })
    expect(wrapper.find('[aria-label^="青角圖騰："]').exists()).toBe(false)
    wrapper.unmount()
  })
})
