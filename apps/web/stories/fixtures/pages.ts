import { configurePage } from '../../.storybook/mocks/page-runtime'
import catalog from './rules-catalog.json'
import { createExampleGameState, exampleCards } from './components'
import { createCompetitionAnnouncementFixture } from '../../tests/fixtures/competition-announcement'
import { tournamentPreview } from './tournament-preview'

export type PageScenario = 'normal' | 'empty' | 'error'
export function setupPageFixture(path: string, scenario: PageScenario = 'normal') {
  let replays = ['青木試陣', '好友切磋'].map((roomName, index) => ({ replayId: `replay-${index}`, sourceGameId: `game-${index}`, roomName, players: [{ player: 'alice', displayName: '小明' }, { player: 'bob', displayName: '小華' }], finishedAt: '2026-10-23 14:30', savedAt: '2026-10-23 14:30', result: { status: 'Finished' } }))
  const state = createExampleGameState()
  state.status = 'InProgress'
  state.environment = 'Wood'
  state.phase = 'Action'
  state.currentPlayer = 'alice'
  state.hands = [{ player: 'alice', cards: { kind: 'known', cards: exampleCards } }, { player: 'bob', cards: { kind: 'hidden', count: 4 } }]
  const announcement = createCompetitionAnnouncementFixture({ teamSize: 1, initialHp: 200, registrationDeadline: 1_820_000_040_000, checkInDeadline: 1_820_003_640_000 })
  configurePage({ path, params: { id: 'competition-a', replayId: 'replay-0' }, fetch: async (url, options) => {
    if (scenario === 'error' && url !== '/api/auth-capabilities') throw new Error('目前無法取得資料，請稍後再試。')
    if (url === '/api/auth-capabilities') return { providers: { google: true, github: true } }
    if (url === '/api/deck') return { source: 'preconstructed', deck: structuredClone(options.method === 'PUT' ? options.body : catalog.deckComposition.personalDeck.preconstructed) }
    if (url === '/api/account/auth-methods') return { providers: { google: true, github: true }, accounts: [{ id: 'credential', providerId: 'credential' }, { id: 'google', providerId: 'google' }], isAnonymous: false, canUpgrade: false }
    if (url === '/api/account/unlink') return {}
    if (url === '/api/replays') return { replays: scenario === 'empty' ? [] : structuredClone(replays) }
    if (url.startsWith('/api/replays/')) {
      if (options.method === 'DELETE') { replays = replays.filter(replay => !url.endsWith(replay.replayId)); return {} }
      const step = Math.min(8, Number(options.query?.step ?? 0))
      return { currentStep: step, totalSteps: 8, state: { ...structuredClone(state), turnNumber: Math.floor(step / 2) + 1 }, firstPlayer: 'alice', players: [{ player: 'alice', displayName: '小明' }, { player: 'bob', displayName: '小華' }], battleRecord: { preparation: { entries: [{ id: 'ready', title: '對局準備完成' }] }, turns: [{ turnNumber: 1, title: '第 1 回合 · 小明', entries: [{ id: 'turn-1', title: '進行回合抽牌', summary: '選擇一張卡牌捨棄。' }] }] } }
    }
    if (url.endsWith('/commands')) return { competitionId: 'competition-a', revision: 2, projection: { ok: true, operation: 'registrationView', public: { announcement, lifecycle: 'open', revision: 2, teams: [{ teamId: 'team-a', captain: 'captain-a', members: ['captain-a'], rosterVersion: 1, confirmedMembers: ['captain-a'], status: 'registered', exclusionReasons: [] }] } } }
    if (url.endsWith('/tournament')) return tournamentPreview()
    throw new Error(`故事未提供此操作的資料：${url}`)
  } })
}
