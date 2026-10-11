import { describe, expect, it } from 'bun:test'
import {
  canSetCompetitionReady,
  competitionReadyErrorMessage,
  competitionWaitingMemberStatus,
  isExactCompetitionReadyAck,
  parseGameRoomCompetitionMode,
} from './competition-ready'

const competitionContext = {
  competitionId: 'competition-1',
  matchId: 'match-1',
  gameNumber: 1,
}

describe('parseGameRoomCompetitionMode', () => {
  it('treats a room response without server competition context as ordinary', () => {
    expect(parseGameRoomCompetitionMode({ gameId: 'ordinary', metadata: { name: '競賽' } }))
      .toEqual({ kind: 'ordinary' })
  })

  it('copies only an exact valid server-projected competition context', () => {
    const source = { ...competitionContext }
    const mode = parseGameRoomCompetitionMode({ competition: source })
    source.competitionId = 'mutated'

    expect(mode).toEqual({ kind: 'competition', context: competitionContext })
    expect(mode.kind === 'competition' && mode.context).not.toBe(source)
  })

  it('fails closed for every malformed present context instead of falling back to ordinary mode', () => {
    const invalidContexts: unknown[] = [
      null,
      [],
      {},
      { ...competitionContext, competitionId: '' },
      { ...competitionContext, competitionId: ' competition-1' },
      { ...competitionContext, matchId: ' ' },
      { ...competitionContext, gameNumber: 0 },
      { ...competitionContext, gameNumber: 1.5 },
      { ...competitionContext, gameNumber: Number.POSITIVE_INFINITY },
      { ...competitionContext, privateHistory: ['secret'] },
    ]

    for (const context of invalidContexts) {
      expect(parseGameRoomCompetitionMode({ competition: context })).toEqual({ kind: 'invalid' })
    }

    const symbolContext = { ...competitionContext, [Symbol('private')]: true }
    expect(parseGameRoomCompetitionMode({ competition: symbolContext })).toEqual({ kind: 'invalid' })
  })

  it('does not infer competition mode from non-authoritative URL-like or metadata fields', () => {
    expect(parseGameRoomCompetitionMode({
      gameId: 'competition-match-game',
      metadata: { name: '競賽房間', gameId: 'competition-room' },
      query: { competitionId: 'competition-1' },
    })).toEqual({ kind: 'ordinary' })
  })
})

describe('canSetCompetitionReady', () => {
  const eligible = {
    mode: { kind: 'competition' as const, context: competitionContext },
    roomStatus: 'Waiting',
    sessionUserId: 'member-a',
    memberUserId: 'member-a',
    memberConnected: true,
    socketConnected: true,
    loading: false,
  }

  it('allows any connected fixed member including the room owner; there is no owner bypass', () => {
    for (const memberUserId of ['owner', 'member-b', 'member-c', 'member-d']) {
      expect(canSetCompetitionReady({
        ...eligible,
        sessionUserId: memberUserId,
        memberUserId,
      })).toBe(true)
    }
  })

  it('rejects ordinary or malformed mode, missing/mismatched identity, inactive lifecycle, disconnection, and loading', () => {
    const blocked = [
      { ...eligible, mode: { kind: 'ordinary' as const } },
      { ...eligible, mode: { kind: 'invalid' as const } },
      { ...eligible, sessionUserId: null },
      { ...eligible, memberUserId: 'member-b' },
      { ...eligible, roomStatus: 'Active' },
      { ...eligible, memberConnected: false },
      { ...eligible, socketConnected: false },
      { ...eligible, loading: true },
    ]

    for (const input of blocked) expect(canSetCompetitionReady(input)).toBe(false)
  })
})

describe('competitionWaitingMemberStatus', () => {
  it('shows readiness for every competitive seat including owner and preserves ordinary owner wording', () => {
    const competitionMode = { kind: 'competition' as const, context: competitionContext }
    expect(competitionWaitingMemberStatus({ ready: true, connected: true, owner: true }, competitionMode))
      .toBe('已準備')
    expect(competitionWaitingMemberStatus({ ready: false, connected: true, owner: true }, competitionMode))
      .toBe('未準備')
    expect(competitionWaitingMemberStatus({ ready: false, connected: false, owner: true }, competitionMode))
      .toBe('未連線')
    expect(competitionWaitingMemberStatus({ ready: true, connected: true, owner: true }, { kind: 'ordinary' }))
      .toBe('房主')
    expect(competitionWaitingMemberStatus(undefined, { kind: 'invalid' })).toBe('等待中')
    expect(competitionWaitingMemberStatus({ ready: true, connected: true, owner: false }, { kind: 'invalid' }))
      .toBe('狀態無法確認')
  })
})

describe('isExactCompetitionReadyAck', () => {
  it('accepts only the exact explicit desired state', () => {
    expect(isExactCompetitionReadyAck({ ready: true }, true)).toBe(true)
    expect(isExactCompetitionReadyAck({ ready: false }, false)).toBe(true)
    expect(isExactCompetitionReadyAck({ ready: false }, true)).toBe(false)
  })

  it('rejects malformed, non-boolean, extra/private, and symbol-bearing responses', () => {
    expect(isExactCompetitionReadyAck(null, true)).toBe(false)
    expect(isExactCompetitionReadyAck({ ready: 'true' }, true)).toBe(false)
    expect(isExactCompetitionReadyAck({ ready: true, private: 'secret' }, true)).toBe(false)
    expect(isExactCompetitionReadyAck({ ready: true, [Symbol('extra')]: true }, true)).toBe(false)
  })
})

describe('competitionReadyErrorMessage', () => {
  it('uses fixed safe messages and never reflects response internals', () => {
    expect(competitionReadyErrorMessage({ statusCode: 403, message: 'rpc-secret' }))
      .toBe('你目前無法變更這場競賽的準備狀態。')
    expect(competitionReadyErrorMessage({ statusCode: 409, data: { private: 'rpc-secret' } }))
      .toBe('房間狀態已改變，請重新查詢目前狀態。')
    expect(competitionReadyErrorMessage(new Error('network-secret')))
      .toBe('準備狀態尚未確認，可使用相同狀態重試。')
  })
})
