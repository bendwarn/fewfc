import { expect, test } from 'bun:test'
import type { CompetitionCorrectionMatch } from './competition-ruling-correction'
import {
  captainNextGameChoice,
  isCaptainNextGameCommand,
  parseCaptainNextGameAcknowledgement,
  type CaptainNextGameCommand,
} from './competition-next-game'

function matchView(
  ruling: unknown,
  overrides: Record<string, unknown> = {},
): CompetitionCorrectionMatch {
  return {
    competitionId: 'competition-1',
    matchId: 'match-1',
    matchRevision: 12,
    match: {
      config: { teams: ['team-a', 'team-b'] },
      status: 'awaitingFirstPlayer',
      games: [{ gameRecordId: 'game-1', rulingRecordId: 'ruling-1' }],
      rulings: [{ gameRecordId: 'game-1', rulingRecordId: 'ruling-1', ruling }],
      nextGame: {
        gameNumber: 2,
        kind: { type: 'ordinaryNewGame' },
        firstPlayer: { type: 'pending', reason: '隊長選擇' },
      },
      ...overrides,
    },
  } as unknown as CompetitionCorrectionMatch
}

test('shows ordinary next-game choice only to the captain of the competitively losing team', () => {
  for (const ruling of [
    { type: 'gameAccepted', outcome: { winner: 'team-a' } },
    { type: 'timeoutLoss', loser: 'team-b' },
    { type: 'eliminationDrawLoss', loser: 'team-b', reason: 'enteredByoyomiFirst' },
  ]) {
    expect(captainNextGameChoice(matchView(ruling), 'team-b', true)).toEqual({
      matchRevision: 12,
      gameNumber: 2,
      teams: ['team-a', 'team-b'],
    })
  }

  expect(captainNextGameChoice(
    matchView({ type: 'gameAccepted', outcome: { winner: 'team-a' } }), 'team-a', true,
  )).toBeNull()
  expect(captainNextGameChoice(
    matchView({ type: 'gameAccepted', outcome: { winner: 'team-a' } }), 'team-b', false,
  )).toBeNull()
  expect(captainNextGameChoice(matchView({ type: 'gameAccepted', outcome: 'draw' }), 'team-b', true)).toBeNull()
})

test('does not offer captain selection for a completed state, a tiebreak, or an assigned first player', () => {
  const ruling = { type: 'timeoutLoss', loser: 'team-b' }
  expect(captainNextGameChoice(matchView(ruling, { status: 'open' }), 'team-b', true)).toBeNull()
  expect(captainNextGameChoice(matchView(ruling, {
    nextGame: {
      gameNumber: 2,
      kind: { type: 'eliminationDrawContinuation', continuation: { teams: [] } },
      firstPlayer: { type: 'assigned', team: 'team-a' },
    },
  }), 'team-b', true)).toBeNull()
  expect(captainNextGameChoice(matchView(ruling, {
    nextGame: {
      gameNumber: 2,
      kind: { type: 'ordinaryNewGame' },
      firstPlayer: { type: 'assigned', team: 'team-a' },
    },
  }), 'team-b', true)).toBeNull()
})

test('accepts only an exact captain next-game command and matching acknowledgement', () => {
  const command: CaptainNextGameCommand = {
    selectedFirstPlayerTeam: 'team-a',
    expectedMatchRevision: 12,
    commandId: 'captain-choice-1',
  }
  expect(isCaptainNextGameCommand(command)).toBe(true)
  expect(isCaptainNextGameCommand({ ...command, firstPlayer: 'team-a' })).toBe(false)
  expect(isCaptainNextGameCommand({ ...command, expectedMatchRevision: Number.MAX_SAFE_INTEGER })).toBe(false)

  expect(parseCaptainNextGameAcknowledgement({
    competitionId: 'competition-1',
    matchId: 'match-1',
    matchRevision: 13,
    gameNumber: 2,
    gameId: 'game-2',
    commandId: command.commandId,
    status: 'pending',
  }, 'competition-1', 'match-1', command)).toEqual({
    gameNumber: 2,
    gameId: 'game-2',
    status: 'pending',
  })
  expect(() => parseCaptainNextGameAcknowledgement({
    competitionId: 'competition-1',
    matchId: 'match-1',
    matchRevision: 14,
    gameNumber: 2,
    gameId: 'game-2',
    commandId: command.commandId,
    status: 'pending',
  }, 'competition-1', 'match-1', command)).toThrow()
})
