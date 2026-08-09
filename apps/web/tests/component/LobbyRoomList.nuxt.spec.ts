import { mountSuspended } from '@nuxt/test-utils/runtime'
import { expect, it } from 'vitest'
import LobbyRoomList from '~/components/LobbyRoomList.vue'

it('renders a rule difference only for rooms whose configuration differs from the catalog default', async () => {
  const wrapper = await mountSuspended(LobbyRoomList, {
    props: {
      heading: '我的房間',
      badge: 2,
      emptyMessage: '尚未加入任何房間。',
      loading: false,
      busy: false,
      items: [
        {
          gameId: 'all-enabled',
          code: 'all-enab',
          name: '完整規則房',
          detail: '1 / 2 玩家 · 等待中',
          ruleSummary: '',
          actionLabel: '進入',
        },
        {
          gameId: 'without-pouch',
          code: 'without-',
          name: '停用錦囊房',
          detail: '1 / 2 玩家 · 等待中',
          ruleSummary: '停用：錦囊',
          actionLabel: '進入',
        },
      ],
    },
  })

  const allEnabled = wrapper.get('button')
  expect(allEnabled.text()).not.toContain('停用：')
  expect(wrapper.text()).toContain('停用：錦囊')
  expect(wrapper.findAll('.public-room-list small')).toHaveLength(3)
})
