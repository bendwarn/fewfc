import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { onBeforeUnmount } from 'vue'
import { expect, within } from 'storybook/test'
import CompetitionTournamentView from '~/components/CompetitionTournamentView.vue'
import PlayerShell from '~/components/PlayerShell.vue'
import { configurePage } from '../../.storybook/mocks/page-runtime'
import { useColorTheme } from '../../.storybook/mocks/useColorTheme'
import { tournamentCompletionPreview, type TournamentCompletionScenario } from '../fixtures/tournament-preview'

const meta = {
  title: '競賽/目前賽程',
  component: CompetitionTournamentView,
  tags: ['autodocs', 'tournament-completion'],
  args: {
    competitionId: 'cup-demo',
    sessionIdentity: null,
    contextRevision: null,
    contextReady: false,
    registrationLocked: false,
    participantTeamId: null,
    participantIsCaptain: false,
    participantEntryEnabled: false,
  },
} satisfies Meta<typeof CompetitionTournamentView>

export default meta
type Story = StoryObj<typeof meta>

export const ContextLoading: Story = {}

function tournamentStory(scenario: TournamentCompletionScenario, dark = false): Story {
  const expected = {
    pending: '賽果：尚無賽果',
    singleFinal: '決賽雙負，冠軍空缺',
    singleBronze: '季軍賽雙負，第三名與第四名空缺',
    singleSemifinal: '半決賽雙負後，唯一勝者輪空取得冠軍；正常敗者輪空取得季軍，亞軍空缺',
    doubleFinal: '總決賽雙負，冠軍與亞軍空缺；保留已決出的季軍',
    doubleReset: '重置賽雙負，冠軍與亞軍空缺；保留已決出的季軍',
    zeroAndOne: '階段已取消',
    laterActive: '決賽雙負，冠軍空缺',
  }[scenario]
  return {
    tags: ['tournament-completion'],
    parameters: { layout: 'fullscreen' },
    render: () => ({
      components: { CompetitionTournamentView, PlayerShell },
      setup() {
        configurePage({ path: '/competitions/competition-a', fetch: async () => tournamentCompletionPreview(scenario) })
        useColorTheme().setPreference(dark ? 'dark' : 'light')
        onBeforeUnmount(() => useColorTheme().setPreference('light'))
      },
      template: `<PlayerShell display-name="小明" connection-state="connected" :notifications="[]">
        <main style="max-width: 76rem; margin: 0 auto; padding: 1rem">
          <CompetitionTournamentView competition-id="competition-a" session-identity="player-story"
            :context-revision="1" :context-ready="true" />
        </main>
      </PlayerShell>`,
    }),
    play: async ({ canvasElement }) => {
      const canvas = within(canvasElement)
      await expect(await canvas.findByText(expected)).toBeVisible()
      if (scenario === 'pending') await expect(canvas.getAllByText('排名未確定')).toHaveLength(2)
      if (scenario === 'singleSemifinal') await expect(canvas.getByText('第 4 名因雙負空缺')).toBeVisible()
      if (scenario === 'singleFinal' || scenario === 'doubleFinal' || scenario === 'doubleReset') {
        await expect(canvas.getByText('第 3 名')).toBeVisible()
        await expect(canvas.queryByText('排名未確定')).not.toBeInTheDocument()
      }
      if (scenario === 'laterActive') {
        await expect(canvas.getAllByText('排名未確定')).toHaveLength(2)
        await expect(canvas.queryByText('本場賽事已完成')).not.toBeInTheDocument()
      }
    },
  }
}

export const Pending = tournamentStory('pending')
export const SingleFinalVacancy = tournamentStory('singleFinal')
export const SingleBronzeVacancy = tournamentStory('singleBronze')
export const SingleSemifinalVacancy = tournamentStory('singleSemifinal')
export const DoubleFinalVacancy = tournamentStory('doubleFinal')
export const DoubleResetVacancy = tournamentStory('doubleReset')
export const ZeroAndOneTeam = tournamentStory('zeroAndOne')
export const SettledThenActive = tournamentStory('laterActive')
export const SingleFinalVacancyDark = tournamentStory('singleFinal', true)
export const DoubleResetVacancyDark = tournamentStory('doubleReset', true)
