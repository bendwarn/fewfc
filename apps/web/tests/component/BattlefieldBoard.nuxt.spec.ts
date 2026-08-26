import { mountSuspended } from '@nuxt/test-utils/runtime'
import { afterEach, describe, expect, it } from 'vitest'
import { h } from 'vue'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import GameConclusionPanel from '~/components/GameConclusionPanel.vue'
import { emptyPublicState } from '#shared/game-room'

const fireThree = {
  id: 7,
  label: '火 3',
  element: 'Fire' as const,
  level: 3,
  secretStrategies: [],
}

afterEach(() => {
  document.querySelectorAll('.battlefield-detail-portal').forEach(element => element.remove())
})

describe('BattlefieldBoard', () => {
  it('keeps replay-specific hand inspection inside the shared board module', async () => {
    const state = emptyPublicState(['alice', 'bob'])
    state.currentPlayer = 'bob'
    state.hands[1]!.cards = { kind: 'known', cards: [fireThree] }
    state.statuses.push({
      id: 'cannot-act',
      owner: { kind: 'player', id: 'bob' },
      kind: 'CannotAct',
      presentation: 'cannotAct',
      duration: { type: 'permanent' },
    })

    const wrapper = await mountSuspended(BattlefieldBoard, {
      attachTo: document.body,
      props: {
        state,
        displayNames: { alice: '小明', bob: '小華' },
        anchorPlayer: 'alice',
        mode: 'replay',
      },
    })

    expect(wrapper.findAll('.player-seat')).toHaveLength(2)
    expect(wrapper.get('.player-seat[aria-current="true"]').attributes('aria-label')).toContain('小華')
    expect(wrapper.get('.effect-summary').text()).toBe('效果 1')
    const handButton = wrapper.get('.compact-hand-button')
    expect(handButton.text()).toBe('手牌 1')
    await handButton.trigger('click')
    expect(document.querySelector('[role="dialog"] h2')?.textContent).toBe('小華的手牌 1')
    expect(document.querySelectorAll('.hand-detail-cards .playing-card')).toHaveLength(1)
  })

  it('switches Personal discard controls between a historical face and pile counters', async () => {
    const state = emptyPublicState(['alice', 'bob'])
    state.enabledRuleModules = ['personal-deck']
    state.turnNumber = 3
    state.playerDecks = [
      { player: 'alice', cards: { kind: 'hidden', count: 12 } },
      { player: 'bob', cards: { kind: 'hidden', count: 10 } },
    ]
    state.playerDiscards = [
      { player: 'alice', cards: [fireThree] },
      { player: 'bob', cards: [] },
    ]
    state.lastCompletedTurnDiscards = [
      { player: 'alice', card: fireThree, turnNumber: 1 },
    ]

    const wrapper = await mountSuspended(BattlefieldBoard, {
      props: {
        state,
        displayNames: { alice: '小明', bob: '小華' },
        anchorPlayer: 'alice',
        mode: 'live',
      },
    })

    expect(wrapper.findAll('.seat-discard-control')).toHaveLength(2)
    expect(wrapper.findAll('.featured-discard-card')).toHaveLength(1)
    expect(wrapper.get('.seat-top .discard-counts').text()).toBe('0—10')
    expect(wrapper.find('.shared-discard-dock').exists()).toBe(false)
  })
})

describe('GameConclusionPanel', () => {
  it('owns canonical conclusion wording when rendered through the board overlay', async () => {
    const state = emptyPublicState(['alice', 'bob'])
    const winningTeam = state.players[0]!.team
    const losingTeam = state.players[1]!.team
    state.status = 'Finished'
    state.gameConclusion = {
      outcome: { type: 'winner', team: winningTeam },
      causes: [{ type: 'teamHpDepleted', teams: [losingTeam] }],
    }

    const wrapper = await mountSuspended(BattlefieldBoard, {
      props: {
        state,
        displayNames: { alice: '小明', bob: '小華' },
        anchorPlayer: 'alice',
        mode: 'replay',
      },
      slots: {
        'board-overlay': h(
          GameConclusionPanel,
          {
            class: 'battlefield-conclusion',
            state,
            teamLabel: (team) => team === winningTeam ? '我方' : '對方',
            summary: '小明先手，本局已結束。',
          },
          { actions: () => h('button', { class: 'primary-button', type: 'button' }, '返回房間') },
        ),
      },
    })

    expect(wrapper.get('.board-center .result-panel').classes()).toContain('battlefield-conclusion')
    expect(wrapper.get('.result-panel h2').text()).toBe('我方 勝利')
    expect(wrapper.get('.result-reason').text()).toBe('終局原因對方 的生命值歸零')
    expect(wrapper.text()).toContain('小明先手，本局已結束。')
    const action = wrapper.get('.result-actions button')
    expect(action.text()).toBe('返回房間')
    expect(action.classes()).toContain('primary-button')
  })
})
