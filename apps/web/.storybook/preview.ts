import '../app/assets/css/main.css'
import { useColorTheme } from './mocks/useColorTheme'
import { setup, type Preview } from '@storybook/vue3-vite'
import { defineComponent, h } from 'vue'
import RouteStatus from '../app/components/RouteStatus.vue'
import PageIntro from '../app/components/PageIntro.vue'
import PublicHeader from '../app/components/PublicHeader.vue'
import ConfirmDialog from '../app/components/ConfirmDialog.vue'
import ModalShell from '../app/components/ModalShell.vue'
import BattlefieldBoard from '../app/components/BattlefieldBoard.vue'
import BattleRecordTimeline from '../app/components/BattleRecordTimeline.vue'
import ReplayEventSheet from '../app/components/ReplayEventSheet.vue'
import GameConclusionPanel from '../app/components/GameConclusionPanel.vue'
import CompetitionPersonalDeckSubmission from '../app/components/CompetitionPersonalDeckSubmission.vue'
import CompetitionTournamentView from '../app/components/CompetitionTournamentView.vue'
import CompetitionTournamentManagement from '../app/components/CompetitionTournamentManagement.vue'
import CompetitionRulingCorrection from '../app/components/CompetitionRulingCorrection.vue'

setup(app => {
  for (const [name, component] of Object.entries({ RouteStatus, PageIntro, PublicHeader, ConfirmDialog, ModalShell, BattlefieldBoard, BattleRecordTimeline, ReplayEventSheet, GameConclusionPanel, CompetitionPersonalDeckSubmission, CompetitionTournamentView, CompetitionTournamentManagement, CompetitionRulingCorrection })) app.component(name, component)
  app.component('NuxtLink', defineComponent({
    props: { to: { type: [String, Object], default: '/' } },
    setup(props, { slots }) {
      return () => h('a', { href: typeof props.to === 'string' ? props.to : '/' }, slots.default?.())
    },
  }))
})

const preview: Preview = {
  parameters: {
    backgrounds: { default: '遊戲介面' },
    layout: 'padded',
  },
  beforeEach: () => {
    useColorTheme().setPreference('system')
  },
  decorators: [story => ({
    components: { story },
    template: '<main style="min-height: 100vh; background: var(--app-canvas); color: var(--app-text)"><story /></main>',
  })],
}

export default preview
