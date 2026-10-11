import { describe, expect, test } from 'bun:test'
import {
  CompetitionParticipantGameProtocolError,
  canShowCompetitionParticipantGameEntry,
  competitionParticipantGameErrorMessage,
  competitionParticipantRoomPath,
  normalizeCompetitionParticipantGameId,
  parseCompetitionParticipantGameResponse,
} from './competition-participant-game'

const currentGame = {
  competitionId: 'competition-a',
  matchId: 'match-a',
  matchRevision: 7,
  game: { gameId: 'room/a', gameNumber: 2 },
}

describe('competition participant-game projection', () => {
  test('allows only locked checked-in teams and listed members to see the entry', () => {
    const onePlayerTeam = { status: 'checkedIn' as const, members: ['member-a'] }
    const twoPlayerTeam = { status: 'checkedIn' as const, members: ['captain-a', 'member-b'] }

    expect(canShowCompetitionParticipantGameEntry('locked', 'member-a', onePlayerTeam)).toBe(true)
    expect(canShowCompetitionParticipantGameEntry('locked', 'member-b', twoPlayerTeam)).toBe(true)
    for (const status of ['registered', 'notCheckedIn', 'withdrawn'] as const) {
      expect(canShowCompetitionParticipantGameEntry('locked', 'member-a', { ...onePlayerTeam, status })).toBe(false)
    }
    expect(canShowCompetitionParticipantGameEntry('locked', 'outsider', twoPlayerTeam)).toBe(false)
    expect(canShowCompetitionParticipantGameEntry('open', 'member-a', onePlayerTeam)).toBe(false)
    expect(canShowCompetitionParticipantGameEntry('draft', 'member-a', onePlayerTeam)).toBe(false)
    expect(canShowCompetitionParticipantGameEntry('locked', null, onePlayerTeam)).toBe(false)
    expect(canShowCompetitionParticipantGameEntry('locked', '  ', onePlayerTeam)).toBe(false)
    expect(canShowCompetitionParticipantGameEntry('locked', 'member-a', null)).toBe(false)
  })

  test('normalizes only path identities and accepts the exact minimal current-game DTO', () => {
    expect(normalizeCompetitionParticipantGameId('  match-a  ')).toBe('match-a')
    expect(normalizeCompetitionParticipantGameId('  ')).toBeNull()
    expect(normalizeCompetitionParticipantGameId(12)).toBeNull()
    expect(parseCompetitionParticipantGameResponse(
      currentGame,
      ' competition-a ',
      ' match-a ',
    )).toEqual(currentGame)
    expect(competitionParticipantRoomPath(currentGame.game)).toBe('/rooms/room%2Fa')
  })

  test('accepts an exact successful null projection without inventing a match status', () => {
    const projection = parseCompetitionParticipantGameResponse({
      competitionId: 'competition-a',
      matchId: 'match-a',
      matchRevision: 1,
      game: null,
    }, 'competition-a', 'match-a')

    expect(projection.game).toBeNull()
    expect(competitionParticipantRoomPath(projection.game)).toBeNull()
  })

  test('rejects forged identities, private extras, and malformed revision or game values', () => {
    const invalid: unknown[] = [
      { ...currentGame, callerUserId: 'secret' },
      { ...currentGame, competitionId: 'other-competition' },
      { ...currentGame, matchId: 'other-match' },
      { ...currentGame, matchRevision: 0 },
      { ...currentGame, matchRevision: Number.MAX_SAFE_INTEGER + 1 },
      { ...currentGame, game: { ...currentGame.game, internalHistory: [] } },
      { ...currentGame, game: { ...currentGame.game, gameId: ' room/a ' } },
      { ...currentGame, game: { ...currentGame.game, gameId: '' } },
      { ...currentGame, game: { ...currentGame.game, gameNumber: 0 } },
      { ...currentGame, game: { ...currentGame.game, gameNumber: 1.5 } },
    ]

    for (const value of invalid) {
      expect(() => parseCompetitionParticipantGameResponse(value, 'competition-a', 'match-a'))
        .toThrow(CompetitionParticipantGameProtocolError)
    }
    expect(() => parseCompetitionParticipantGameResponse(currentGame, '', 'match-a'))
      .toThrow(CompetitionParticipantGameProtocolError)
  })

  test('maps errors to fixed safe Chinese messages without exposing server details', () => {
    const secret = 'private-rpc-detail-do-not-render'
    expect(competitionParticipantGameErrorMessage({ statusCode: 403, message: secret, data: { code: secret } }))
      .toBe('你沒有這場競賽對局的存取權。')
    expect(competitionParticipantGameErrorMessage({ status: 503, message: secret }))
      .toBe('競賽服務暫時無法使用，請稍後重試。')
    expect(competitionParticipantGameErrorMessage(new Error(secret))).toBe('暫時無法查詢競賽對局，請檢查連線後重試。')
    expect(competitionParticipantGameErrorMessage({ statusCode: 404, message: secret })).not.toContain(secret)
  })
})
