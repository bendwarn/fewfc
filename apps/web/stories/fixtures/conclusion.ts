import { createExampleGameState, createFourPlayerGameState } from './components'
import type { PublicGameState } from '~/types/fewfc'

// 固定終局公開資料，用於結算版面；不在瀏覽器推導勝負。
export function createConclusionState(kind: 'victory' | 'draw' | 'direct' | 'automatic' | 'retrieval' | 'no-resolution' | 'hidden' = 'victory', fourPlayers = false): PublicGameState {
  const state = fourPlayers ? createFourPlayerGameState() : createExampleGameState()
  const winningTeam = state.players[0]!.team
  const losingTeam = state.players[1]!.team
  state.status = 'Finished'
  state.environment = 'Wood'
  state.turnNumber = 8
  state.phase = 'Action'
  state.currentPlayer = 'alice'
  state.pendingChoice = null
  state.pendingRandomness = null
  state.deckCount = 24
  state.hp = [{ team: winningTeam, hp: 64 }, { team: losingTeam, hp: 0 }]
  state.initialHp = state.hp.map(entry => ({ ...entry, hp: 100 }))
  state.gameConclusion = {
    outcome: { type: 'winner', team: winningTeam },
    causes: [{ type: 'teamHpDepleted', teams: [losingTeam] }],
  }
  state.terminalResolution = {
    type: 'formation', player: 'alice', formationId: 'fire-attack', formationName: '烈焰陣',
    cards: { kind: 'known', cards: [1, 2, 3].map((level, index) => ({ id: 51 + index, label: `火 ${level}`, element: 'Fire', level, secretStrategies: [] })) },
  }
  if (kind === 'draw') {
    state.hp = state.hp.map(entry => ({ ...entry, hp: 0 }))
    state.gameConclusion = {
      outcome: { type: 'draw' },
      causes: [{ type: 'teamHpDepleted', teams: [winningTeam, losingTeam] }],
    }
  } else if (kind === 'direct') {
    state.hp[1]!.hp = 82
    state.gameConclusion = {
      outcome: { type: 'winner', team: winningTeam },
      causes: [{ type: 'directVictory', rule: 'five-star-alignment', team: winningTeam }],
    }
    state.terminalResolution = { type: 'automatic', label: '五星連珠' }
  } else if (kind === 'automatic') {
    state.terminalResolution = { type: 'automatic', label: '中毒效果' }
  } else if (kind === 'retrieval') {
    state.terminalResolution = { type: 'discardRetrieval', player: 'alice', previousPlayer: 'bob', card: { id: 60, label: '火 3', element: 'Fire', level: 3, secretStrategies: [] } }
  } else if (kind === 'no-resolution') {
    state.terminalResolution = null
  } else if (kind === 'hidden') {
    state.terminalResolution.cards = { kind: 'partiallyKnown', cards: [state.terminalResolution.cards.kind === 'known' ? state.terminalResolution.cards.cards[0]! : null, null] }
  }
  return state
}
