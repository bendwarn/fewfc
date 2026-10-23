import { useColorTheme } from '../../.storybook/mocks/useColorTheme'
import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { onBeforeUnmount, ref } from 'vue'
import PlayerShell from '~/components/PlayerShell.vue'
import WaitingRoomLayout from '~/components/WaitingRoomLayout.vue'
import RuleModuleSettings from '~/components/RuleModuleSettings.vue'
import { allRuleModulesEnabled, ruleModuleCatalog } from '../fixtures/rule-modules'

const meta = {
  title: '房間/等待室版面',
  component: WaitingRoomLayout,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof WaitingRoomLayout>
export default meta
type Story = StoryObj<typeof meta>

function room(dark: boolean, fourPlayers = false): Story {
  return {
    render: () => ({
      components: { PlayerShell, WaitingRoomLayout, RuleModuleSettings },
      setup() {
        useColorTheme().setPreference(dark ? 'dark' : 'light')
        onBeforeUnmount(() => { useColorTheme().setPreference('light') })
        return { ruleModuleCatalog, enabled: ref([...allRuleModulesEnabled]), fourPlayers }
      },
      template: `<PlayerShell display-name="小明" connection-state="connected" :notifications="[]">
        <main class="game-page" data-in-game="false" style="display:flex;flex:1;min-height:0">
          <div class="battle-layout" style="position:relative;flex:1;min-height:0">
            <WaitingRoomLayout>
              <template #main>
                <h2>好友切磋</h2><p>等待所有玩家準備完成，即可開始對局。</p>
                <RuleModuleSettings :catalog="ruleModuleCatalog" v-model:enabled-rule-modules="enabled" :is-owner="true" />
              </template>
              <template #side>
                <div class="waiting-members">
                  <span class="joined">小明<small>房主・已連線</small></span>
                  <span class="joined">小華<small>準備完成</small><button type="button">移除</button></span>
                  <template v-if="fourPlayers"><span class="joined">旅人<small>尚未準備</small><button type="button">移除</button></span><span>第四席<small>等待加入</small></span></template>
                </div>
                <div class="result-actions"><button class="ghost-button" type="button">解散</button><button class="primary-button" type="button" :disabled="fourPlayers">開始遊戲 →</button></div>
              </template>
            </WaitingRoomLayout>
          </div>
        </main>
      </PlayerShell>`,
    }),
  }
}
export const Default = room(false)
export const Dark = room(true)
export const FourPlayers = room(false, true)
export const FourPlayersDark = room(true, true)
