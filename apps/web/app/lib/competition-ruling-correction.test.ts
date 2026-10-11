import { describe, expect, test } from 'bun:test'
import type { CompetitionRuling } from '../../shared/competition'
import {
  buildCompetitionRulingCorrection,
  competitionCorrectionCandidates,
  competitionCorrectionPath,
  createCompetitionCorrectionScope,
  isAssignedCompetitionReferee,
  parseCompetitionCorrectionAck,
  parseCompetitionCorrectionMatch,
  keepCompetitionCorrectionIntent,
  competitionRulingLabel,
  CompetitionCorrectionProtocolError,
  competitionCorrectionErrorMessage,
} from './competition-ruling-correction'

const teams: [string, string] = ['b45abe94-a252-4e3d-bff2-7d26c6e2fabc', '535ec115-7bf4-4bbf-b0d3-c6e0ee76c8b7']
const referee = 'referee-a'

// 欄位依 competition_web_api::MatchPublicView/WebGameRecord export，不拿 checkpoint 當 public。
function fixture() {
  return {
    competitionId: 'competition a', matchId: 'match/one', matchRevision: 4,
    matchProjection: {
      ok: true, operation: 'matchSubmitGame',
      public: { match: {
        config: { matchId: 'match/one', teams: [...teams], format: { type: 'fixedGames', value: 1 },
          firstGame: { type: 'assigned', team: teams[0] }, referees: [{ refereeId: referee, conflictedTeam: null }] },
        status: 'completed', score: [1, 0],
        games: [{ gameNumber: 1, gameRecordId: 'game-record', startKind: { type: 'ordinaryNewGame' },
          firstPlayer: teams[0], terminalEvidence: { type: 'canonicalGameEnded', conclusion: {
            outcome: { type: 'winner', team: teams[0] }, causes: [{ type: 'teamHpDepleted', teams: [teams[1]] }],
          } }, rulingRecordId: 'ruling-original' }],
        rulings: [{ rulingRecordId: 'ruling-original', gameRecordId: 'game-record', ruling: { type: 'gameAccepted', outcome: { winner: teams[0] } } }],
        corrections: [], outcome: { type: 'winner', winnerId: teams[0], loserId: teams[1], score: [1, 0] },
        nextFirstPlayer: null, nextGame: null,
      } },
    }, clockProjection: null,
  }
}

function scope() {
  const result = createCompetitionCorrectionScope(' competition a ', ' match/one ', referee)
  if (!result) throw new Error('正常 scope 應成立')
  return result
}

function withMatch(patch: Record<string, unknown>) {
  const value = fixture()
  return { ...value, matchProjection: { ...value.matchProjection, public: { match: { ...value.matchProjection.public.match, ...patch } } } }
}

function rulingFixture(ruling: CompetitionRuling) {
  const base = fixture().matchProjection.public.match
  const timeout = ruling.type === 'timeoutLoss' || ruling.type === 'doubleTimeoutLoss'
  const terminalEvidence = timeout
    ? { type: 'timeout', timeoutId: 'timeout-one', clockCheckpointDigest: 'digest-one', observedAtMonotonicMs: 1000,
      expiries: ruling.type === 'timeoutLoss' ? [{ team: ruling.loser, deadlineMonotonicMs: 1000 }]
        : teams.map(team => ({ team, deadlineMonotonicMs: 1000 })) }
    : { type: 'canonicalGameEnded', conclusion: { outcome: { type: 'draw' }, causes: [{ type: 'teamHpDepleted', teams: [...teams] }] } }
  let patch: Record<string, unknown> = {}
  if (ruling.type === 'doubleTimeoutLoss') patch = { status: 'awaitingReferee', score: [0, 0], outcome: { type: 'doubleLoss', teams: [...teams], progression: { type: 'pendingReferee', reason: '政策未定' } } }
  else if (ruling.type === 'needsDecision') patch = { status: 'awaitingReferee', score: [0, 0], outcome: { type: 'pendingReferee', gameRecordId: 'game-record', reason: ruling.reason } }
  else if (ruling.type === 'needsTiebreak') patch = { status: 'open', score: [0, 0], outcome: null, nextFirstPlayer: teams[1],
    nextGame: { gameNumber: 2, kind: { type: 'eliminationDrawContinuation', continuation: ruling.continuation }, firstPlayer: { type: 'assigned', team: teams[1] } } }
  else if (ruling.type === 'eliminationDrawLoss') patch = { score: [0, 1], outcome: { type: 'winner', winnerId: teams[1], loserId: teams[0], score: [0, 1] } }
  else if (ruling.type === 'gameAccepted') patch = { score: [0, 0], outcome: { type: 'draw', score: [0, 0] } }
  const elimination = ruling.type === 'needsTiebreak' || ruling.type === 'needsDecision' || ruling.type === 'eliminationDrawLoss'
  return withMatch({ ...patch, config: { ...base.config, format: { type: elimination ? 'firstToWins' : 'fixedGames', value: ruling.type === 'needsTiebreak' ? 2 : 1 } },
    games: [{ ...base.games[0], terminalEvidence }], rulings: [{ rulingRecordId: 'ruling-original', gameRecordId: 'game-record', ruling }] })
}

describe('競賽裁判更正 app 契約', () => {
  test('正常交手讀取、明示已保存候選建立七欄固定命令，再驗最小 receipt ACK', () => {
    const identity = scope()
    const view = parseCompetitionCorrectionMatch(fixture(), identity)
    expect(competitionCorrectionPath(identity)).toBe('/api/competitions/competition%20a/matches/match%2Fone')
    expect(isAssignedCompetitionReferee(view, referee)).toBe(true)
    const candidates = competitionCorrectionCandidates(view, 'game-record')
    const saved = candidates.find(candidate => candidate.key === 'saved:ruling-original')
    if (!saved) throw new Error('同局已保存判定應可選擇')
    expect(saved.label).toBe('已保存判定 1：採納原始結論，甲隊獲勝')
    let count = 0
    const body = buildCompetitionRulingCorrection(view, identity, 'game-record', saved.key, '  核對原始結論  ', () => `uuid-${++count}`)
    expect(count).toBe(3)
    expect(body).toEqual({ commandId: 'uuid-1', expectedMatchRevision: 4, correctionId: 'uuid-2',
      gameRecordId: 'game-record', replacementRulingRecordId: 'uuid-3',
      replacementRuling: { type: 'gameAccepted', outcome: { winner: teams[0] } }, reason: '核對原始結論' })
    expect(parseCompetitionCorrectionAck({ competitionId: 'competition a', matchId: 'match/one', matchRevision: 5,
      commandId: 'uuid-1', idempotent: false }, identity, body)).toEqual({ matchRevision: 5, idempotent: false })
  })

  test('同局的更正必須形成完整引用鏈，不能以兩個互相存在的 ID 掩蓋錯接 previous', () => {
    const base = fixture().matchProjection.public.match
    const accepted = { type: 'gameAccepted', outcome: { winner: teams[0] } }
    const rulingTwo = { rulingRecordId: 'ruling-2', gameRecordId: 'game-record', ruling: accepted }
    const rulingThree = { rulingRecordId: 'ruling-3', gameRecordId: 'game-record', ruling: accepted }
    const correctionOne = { correctionId: 'c-1', refereeId: referee, gameRecordId: 'game-record',
      previousRulingRecordId: 'ruling-original', replacementRulingRecordId: 'ruling-2', replacementRuling: accepted, reason: '第一次核對' }
    const correctionTwo = { ...correctionOne, correctionId: 'c-2', previousRulingRecordId: 'ruling-2', replacementRulingRecordId: 'ruling-3', reason: '第二次核對' }
    const patch = { games: [{ ...base.games[0], rulingRecordId: 'ruling-3' }],
      rulings: [...base.rulings, rulingTwo, rulingThree], corrections: [correctionOne, correctionTwo] }
    const valid = parseCompetitionCorrectionMatch(withMatch(patch), scope())
    expect(competitionCorrectionCandidates(valid, 'game-record')).toHaveLength(4)
    expect(() => parseCompetitionCorrectionMatch(withMatch({ ...patch, corrections: [correctionOne, { ...correctionTwo, previousRulingRecordId: 'ruling-original' }] }), scope())).toThrow(CompetitionCorrectionProtocolError)
    expect(() => parseCompetitionCorrectionMatch(withMatch({ ...patch, games: base.games }), scope())).toThrow(CompetitionCorrectionProtocolError)
    expect(() => parseCompetitionCorrectionMatch(withMatch({ ...patch, corrections: [correctionOne] }), scope())).toThrow(CompetitionCorrectionProtocolError)
  })

  test('機器紀錄 ID 即使等於 canonical 也不能使兩個候選共用 key', () => {
    const base = fixture().matchProjection.public.match
    const view = parseCompetitionCorrectionMatch(withMatch({ games: [{ ...base.games[0], rulingRecordId: 'canonical' }],
      rulings: [{ ...base.rulings[0], rulingRecordId: 'canonical' }] }), scope())
    const candidates = competitionCorrectionCandidates(view, 'game-record')
    expect(new Set(candidates.map(candidate => candidate.key)).size).toBe(2)
  })

  test('只用交手 assignment/null conflict 呈現資格，主辦、其他帳號或有衝突不能自動取得', () => {
    const base = fixture().matchProjection.public.match
    const view = parseCompetitionCorrectionMatch(fixture(), scope())
    expect(isAssignedCompetitionReferee(view, 'organizer')).toBe(false)
    expect(isAssignedCompetitionReferee(view, null)).toBe(false)
    const conflicted = parseCompetitionCorrectionMatch(withMatch({ config: { ...base.config, referees: [{ refereeId: referee, conflictedTeam: teams[0] }] } }), scope())
    expect(isAssignedCompetitionReferee(conflicted, referee)).toBe(false)
    expect(() => buildCompetitionRulingCorrection(conflicted, scope(), 'game-record', 'canonical', '理由')).toThrow()
  })

  test('六種已保存判定保持真 payload；沒有任選 winner／新時間／雙敗晉級欄位', () => {
    const cases: { ruling: CompetitionRuling; label: string; canonical: boolean }[] = [
      { ruling: { type: 'timeoutLoss', loser: teams[1] }, label: '乙隊超時判負', canonical: false },
      { ruling: { type: 'doubleTimeoutLoss', teams }, label: '雙方超時判負，後續待裁判處理', canonical: false },
      { ruling: { type: 'gameAccepted', outcome: 'draw' }, label: '採納原始結論，和局', canonical: true },
      { ruling: { type: 'eliminationDrawLoss', loser: teams[0], reason: 'enteredByoyomiFirst' }, label: '甲隊依先進讀秒的和局政策判負', canonical: false },
      { ruling: { type: 'needsTiebreak', continuation: { teams: [{ team: teams[0], mainTimeRemainingMs: 1234 }, { team: teams[1], mainTimeRemainingMs: null }] } }, label: '和局需加賽，沿用已保存的主時間', canonical: false },
      { ruling: { type: 'needsDecision', reason: 'simultaneousByoyomiEntry' }, label: '同時進入讀秒，待裁判裁定', canonical: false },
    ]
    for (const item of cases) {
      const view = parseCompetitionCorrectionMatch(rulingFixture(item.ruling), scope())
      const options = competitionCorrectionCandidates(view, 'game-record')
      const saved = options.find(option => option.key !== 'canonical')
      if (!saved) throw new Error('應有已保存判定')
      expect(saved.ruling).toEqual(item.ruling)
      expect(competitionRulingLabel(saved.ruling, teams)).toBe(item.label)
      expect(options.some(option => option.key === 'canonical')).toBe(item.canonical)
      let count = 0
      const body = buildCompetitionRulingCorrection(view, scope(), 'game-record', saved.key, '核對', () => `copy-${++count}`)
      expect(body.replacementRuling).toEqual(item.ruling)
      expect(body.replacementRuling).not.toBe(item.ruling)
      expect(Object.isFrozen(body.replacementRuling)).toBe(true)
      expect(JSON.stringify(body)).not.toContain('callerUserId')
    }
  })

  test('whole DTO 拒絕錯 identity、extra/private、closed tags、unsafe numbers、重複與錯接關係', () => {
    const root = fixture()
    const base = root.matchProjection.public.match
    const game = base.games[0]
    const bad: (() => unknown)[] = [
      () => ({ ...root, competitionId: 'other' }), () => ({ ...root, matchId: 'other' }),
      () => ({ ...root, callerUserId: referee }), () => ({ ...root, matchRevision: '4' }),
      () => ({ ...root, matchRevision: Number.MAX_SAFE_INTEGER + 1 }), () => ({ ...root, matchRevision: -1 }),
      () => ({ ...root, matchProjection: { ...root.matchProjection, operation: 'clockObserve' } }),
      () => withMatch({ private: { deck: [] } }), () => withMatch({ status: 'finished' }),
      () => withMatch({ config: { ...base.config, matchId: 'wrong' } }),
      () => withMatch({ config: { ...base.config, teams: [teams[0], teams[0]] } }),
      () => withMatch({ config: { ...base.config, referees: [...base.config.referees, ...base.config.referees] } }),
      () => withMatch({ config: { ...base.config, referees: [{ refereeId: referee, conflictedTeam: 'outsider' }] } }),
      () => withMatch({ config: { ...base.config, format: { type: 'fixedGames', value: 0 } } }),
      () => withMatch({ score: [1, NaN] }), () => withMatch({ games: [{ ...game, gameNumber: 0 }] }),
      () => withMatch({ games: [{ ...game, gameNumber: 65536 }] }), () => withMatch({ games: [{ ...game, firstPlayer: 'outsider' }] }),
      () => withMatch({ games: [{ ...game, rulingRecordId: 'missing' }] }), () => withMatch({ games: [game, game] }),
      () => withMatch({ rulings: [...base.rulings, ...base.rulings] }),
      () => withMatch({ rulings: [{ ...base.rulings[0], gameRecordId: 'other' }] }),
      () => withMatch({ games: [{ ...game, terminalEvidence: { type: 'canonicalGameEnded', conclusion: { outcome: { type: 'winner', team: 'other' }, causes: [] } } }] }),
      () => withMatch({ outcome: { type: 'winner', winnerId: teams[0], loserId: teams[1], score: [0, 1] } }),
      () => withMatch({ nextFirstPlayer: 'other' }),
      () => ({ ...root, clockProjection: { ok: true, operation: 'clockObserve', public: { private: 'secret' } } }),
    ]
    for (const input of bad) expect(() => parseCompetitionCorrectionMatch(input(), scope())).toThrow(CompetitionCorrectionProtocolError)
    const symbol = { ...root, [Symbol('private')]: true }
    expect(() => parseCompetitionCorrectionMatch(symbol, scope())).toThrow(CompetitionCorrectionProtocolError)
    const getter = Object.defineProperty({ ...root }, 'matchRevision', { enumerable: true, get: () => { throw new Error('secret') } })
    expect(() => parseCompetitionCorrectionMatch(getter, scope())).toThrow('交手回應無法安全驗證')
  })

  test('timeout 與 continuation 只能沿用完整已保存證據，損壞 payload 不產生候選', () => {
    const base = fixture().matchProjection.public.match
    const terminal = { type: 'timeout', timeoutId: 't', expiries: [{ team: teams[1], deadlineMonotonicMs: 100 }], observedAtMonotonicMs: 100, clockCheckpointDigest: 'd' }
    for (const patch of [
      { ...terminal, expiries: [] }, { ...terminal, observedAtMonotonicMs: 99 },
      { ...terminal, expiries: [{ team: 'other', deadlineMonotonicMs: 100 }] },
      { ...terminal, expiries: [{ team: teams[0], deadlineMonotonicMs: 99 }, { team: teams[1], deadlineMonotonicMs: 100 }] },
      { ...terminal, clockCheckpointDigest: '' },
    ]) expect(() => parseCompetitionCorrectionMatch(withMatch({ games: [{ ...base.games[0], terminalEvidence: patch }] }), scope())).toThrow()
    for (const limit of ['1234', -1, NaN, Infinity]) expect(() => parseCompetitionCorrectionMatch(withMatch({ rulings: [{ rulingRecordId: 'ruling-original', gameRecordId: 'game-record',
      ruling: { type: 'needsTiebreak', continuation: { teams: [{ team: teams[0], mainTimeRemainingMs: limit }, { team: teams[1], mainTimeRemainingMs: null }] } } }] }), scope())).toThrow()
  })

  test('timeout evidence 與每筆已保存 ruling 必須一致；單欄錯 loser 不能成為候選', () => {
    const base = fixture().matchProjection.public.match
    const singleEvidence = { type: 'timeout', timeoutId: 'timeout-one', clockCheckpointDigest: 'digest-one', observedAtMonotonicMs: 100,
      expiries: [{ team: teams[1], deadlineMonotonicMs: 100 }] }
    const doubleEvidence = { ...singleEvidence, expiries: [
      { team: teams[0], deadlineMonotonicMs: 100 }, { team: teams[1], deadlineMonotonicMs: 100 },
    ] }
    const singleRuling: CompetitionRuling = { type: 'timeoutLoss', loser: teams[1] }
    const doubleRuling: CompetitionRuling = { type: 'doubleTimeoutLoss', teams: [...teams] }
    const single = { games: [{ ...base.games[0], terminalEvidence: singleEvidence }],
      rulings: [{ ...base.rulings[0], ruling: singleRuling }] }
    const double = { status: 'awaitingReferee', score: [0, 0],
      outcome: { type: 'doubleLoss', teams: [...teams], progression: { type: 'pendingReferee', reason: '待裁判處理' } },
      games: [{ ...base.games[0], terminalEvidence: doubleEvidence }], rulings: [{ ...base.rulings[0], ruling: doubleRuling }] }
    expect(parseCompetitionCorrectionMatch(withMatch(single), scope()).match?.rulings[0]?.ruling).toEqual(singleRuling)
    expect(parseCompetitionCorrectionMatch(withMatch(double), scope()).match?.rulings[0]?.ruling).toEqual(doubleRuling)
    // core 比較雙方集合，不要求 ruling 的雙隊順序等於 expiry 的順序。
    expect(parseCompetitionCorrectionMatch(withMatch({ ...double, rulings: [{ ...base.rulings[0],
      ruling: { type: 'doubleTimeoutLoss', teams: [teams[1], teams[0]] } }] }), scope()).match?.rulings[0]?.ruling)
      .toEqual({ type: 'doubleTimeoutLoss', teams: [teams[1], teams[0]] })

    const bad: Record<string, unknown>[] = [
      { ...single, rulings: [{ ...base.rulings[0], ruling: { ...singleRuling, loser: teams[0] } }] },
      { ...single, games: [{ ...base.games[0], terminalEvidence: doubleEvidence }] },
      { ...double, games: [{ ...base.games[0], terminalEvidence: singleEvidence }] },
      { ...single, games: base.games }, { ...double, games: base.games },
      ...[
        { type: 'gameAccepted', outcome: { winner: teams[0] } },
        { type: 'eliminationDrawLoss', loser: teams[1], reason: 'enteredByoyomiFirst' },
        { type: 'needsTiebreak', continuation: { teams: [
          { team: teams[0], mainTimeRemainingMs: 100 }, { team: teams[1], mainTimeRemainingMs: null },
        ] } },
        { type: 'needsDecision', reason: 'zeroTimeControl' },
      ].map(ruling => ({ ...single, rulings: [{ ...base.rulings[0], ruling }] })),
      // 目前 ruling 可合法，仍須拒絕更正鏈較早的一筆錯隊伍 ruling。
      { ...single, games: [{ ...base.games[0], terminalEvidence: singleEvidence, rulingRecordId: 'replacement' }],
        rulings: [{ ...base.rulings[0], ruling: { ...singleRuling, loser: teams[0] } },
          { rulingRecordId: 'replacement', gameRecordId: 'game-record', ruling: singleRuling }],
        corrections: [{ correctionId: 'correction', refereeId: referee, gameRecordId: 'game-record', previousRulingRecordId: 'ruling-original',
          replacementRulingRecordId: 'replacement', replacementRuling: singleRuling, reason: '核對' }] },
    ]
    for (const patch of bad) expect(() => parseCompetitionCorrectionMatch(withMatch(patch), scope())).toThrow(CompetitionCorrectionProtocolError)
  })

  test('snapshot 深複製並凍結，read 新版／草稿修改都不能變更 same-body retry', () => {
    const raw = fixture()
    const view = parseCompetitionCorrectionMatch(raw, scope())
    let count = 0
    const body = buildCompetitionRulingCorrection(view, scope(), 'game-record', 'canonical', '最初理由', () => `immutable-${++count}`)
    const serialized = JSON.stringify(body)
    const originalRuling = raw.matchProjection.public.match.rulings[0]
    if (!originalRuling) throw new Error('fixture 應有原始判定')
    originalRuling.ruling.outcome.winner = teams[1]
    if (!view.match) throw new Error('應有 view')
    view.match.rulings.splice(0)
    parseCompetitionCorrectionMatch({ ...fixture(), matchRevision: 99 }, scope())
    expect(JSON.stringify(body)).toBe(serialized)
    expect(Object.isFrozen(body)).toBe(true)
    expect(Object.isFrozen(body.replacementRuling)).toBe(true)
    expect(() => buildCompetitionRulingCorrection(parseCompetitionCorrectionMatch(fixture(), scope()), scope(), 'game-record', 'unknown', '理由')).toThrow()
    expect(() => buildCompetitionRulingCorrection(parseCompetitionCorrectionMatch(fixture(), scope()), scope(), 'game-record', 'canonical', '   ')).toThrow()
    expect(() => buildCompetitionRulingCorrection(parseCompetitionCorrectionMatch({ ...fixture(), matchRevision: Number.MAX_SAFE_INTEGER }, scope()), scope(), 'game-record', 'canonical', '理由')).toThrow()
  })

  test('ACK 必須精確 mirror 原命令與 expected+1，後續 read revision 不能作 ACK', () => {
    let count = 0
    const body = buildCompetitionRulingCorrection(parseCompetitionCorrectionMatch(fixture(), scope()), scope(), 'game-record', 'canonical', '理由', () => `ack-${++count}`)
    const ack = { competitionId: 'competition a', matchId: 'match/one', matchRevision: 5, commandId: body.commandId, idempotent: true }
    expect(parseCompetitionCorrectionAck(ack, scope(), body)).toEqual({ matchRevision: 5, idempotent: true })
    for (const input of [{ ...ack, competitionId: 'other' }, { ...ack, matchId: 'other' }, { ...ack, commandId: 'other' },
      { ...ack, matchRevision: 99 }, { ...ack, matchRevision: '5' }, { ...ack, idempotent: 1 }, { ...ack, private: 'secret' }, fixture()]) {
      expect(() => parseCompetitionCorrectionAck(input, scope(), body)).toThrow()
    }
  })

  test('已有 unknown 後任何後續拒絕不能解決未知；首次明確拒絕才可分類', () => {
    for (const statusCode of [400, 401, 403, 404, 409, 422]) {
      expect(keepCompetitionCorrectionIntent(false, { statusCode })).toBe(false)
      expect(keepCompetitionCorrectionIntent(true, { statusCode })).toBe(true)
    }
    for (const error of [{ statusCode: 503 }, { statusCode: 502 }, new Error('response loss'), new CompetitionCorrectionProtocolError()]) {
      expect(keepCompetitionCorrectionIntent(false, error)).toBe(true)
    }
    const empty = parseCompetitionCorrectionMatch({ ...fixture(), matchProjection: null }, scope())
    expect(isAssignedCompetitionReferee(empty, referee)).toBe(false)
    expect(competitionCorrectionCandidates(empty, 'game-record')).toEqual([])
    expect(createCompetitionCorrectionScope(null, 'match', referee)).toBeNull()
  })

  test('gameAccepted 即使聲稱已保存也須符合該局 canonical，不能拿錯隊伍／超時當可採納勝者', () => {
    const base = fixture().matchProjection.public.match
    expect(() => parseCompetitionCorrectionMatch(withMatch({ rulings: [{ ...base.rulings[0], ruling: { type: 'gameAccepted', outcome: { winner: teams[1] } } }] }), scope())).toThrow()
    expect(() => parseCompetitionCorrectionMatch(withMatch({ rulings: [{ ...base.rulings[0], ruling: { type: 'gameAccepted', outcome: 'draw' } }] }), scope())).toThrow()
    expect(() => parseCompetitionCorrectionMatch(withMatch({ games: [{ ...base.games[0], terminalEvidence: { type: 'timeout', timeoutId: 't', expiries: [{ team: teams[1], deadlineMonotonicMs: 100 }], observedAtMonotonicMs: 100, clockCheckpointDigest: 'd' } }] }), scope())).toThrow()
    const accepted = parseCompetitionCorrectionMatch(fixture(), scope())
    expect(competitionCorrectionCandidates(accepted, 'game-record').find(item => item.key === 'canonical')?.ruling)
      .toEqual({ type: 'gameAccepted', outcome: { winner: teams[0] } })
  })

  test('已確認 ACK 的 readonly refresh error不能另宣稱未知 mutation，未知錯誤不反射原文', () => {
    expect(competitionCorrectionErrorMessage({ statusCode: 503, message: 'secret' }, 'read')).toBe('交手服務暫時無法使用，請手動重新查詢。')
    expect(competitionCorrectionErrorMessage(new CompetitionCorrectionProtocolError(), 'read')).toBe('交手回應無法安全驗證，未顯示投影；請重新查詢。')
    expect(competitionCorrectionErrorMessage({ statusCode: 503, message: 'secret' })).not.toContain('secret')
  })

  test('empty初始交手保持pending先手原狀態；status/outcome/nextGame關係不可互相矛盾', () => {
    const base = fixture().matchProjection.public.match
    const empty = { config: { ...base.config, firstGame: { type: 'pending', reason: '待指定' } }, status: 'awaitingFirstPlayer', score: [0, 0],
      games: [], rulings: [], corrections: [], outcome: null, nextFirstPlayer: null,
      nextGame: { gameNumber: 1, kind: { type: 'ordinaryNewGame' }, firstPlayer: { type: 'pending', reason: '待指定' } } }
    expect(parseCompetitionCorrectionMatch(withMatch(empty), scope()).match?.status).toBe('awaitingFirstPlayer')
    expect(() => parseCompetitionCorrectionMatch(withMatch({ ...empty, nextGame: null }), scope())).toThrow()
    expect(() => parseCompetitionCorrectionMatch(withMatch({ ...empty, nextGame: { ...empty.nextGame, gameNumber: 2 } }), scope())).toThrow()
    expect(() => parseCompetitionCorrectionMatch(withMatch({ nextFirstPlayer: teams[0] }), scope())).toThrow()
    expect(() => parseCompetitionCorrectionMatch(withMatch({ status: 'awaitingReferee', outcome: null }), scope())).toThrow()
  })

  test('不可信錯誤的prototype/getter失敗也只能固定safe文案，保持未知intent', () => {
    const error = new Proxy({}, { getPrototypeOf: () => { throw new Error('private error prototype') } })
    expect(keepCompetitionCorrectionIntent(true, error)).toBe(true)
    expect(keepCompetitionCorrectionIntent(false, error)).toBe(true)
    expect(competitionCorrectionErrorMessage(error, 'read')).toBe('交手服務暫時無法使用，請手動重新查詢。')
    expect(competitionCorrectionErrorMessage(error)).not.toContain('private error prototype')
  })
})
