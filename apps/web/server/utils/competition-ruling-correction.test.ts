import { expect, test } from 'bun:test'
import type { CompetitionRuling } from '../../shared/competition'
import {
  buildRulingCorrectionCommand,
  buildRulingCorrectionRequest,
  projectRulingCorrectionAcknowledgement,
} from './competition-ruling-correction'

const validBody = {
  commandId: ' command-1 ',
  expectedMatchRevision: 4,
  correctionId: ' correction-1 ',
  gameRecordId: ' game-1 ',
  replacementRulingRecordId: ' ruling-2 ',
  replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' } },
  reason: '  裁判覆核  ',
}

test('建構器只接受明確更正欄位，且由呼叫者與伺服器補入裁判及時間', () => {
  const command = buildRulingCorrectionCommand(validBody)
  expect(command).toEqual({
    commandId: 'command-1',
    expectedMatchRevision: 4,
    correctionId: 'correction-1',
    gameRecordId: 'game-1',
    replacementRulingRecordId: 'ruling-2',
    replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' } },
    reason: '裁判覆核',
  })
  expect(buildRulingCorrectionRequest(' comp-1 ', ' match-1 ', ' referee-1 ', command, 900))
    .toEqual({
      competitionId: 'comp-1',
      matchId: 'match-1',
      callerUserId: 'referee-1',
      commandId: 'command-1',
      expectedMatchRevision: 4,
      trustedTimeMs: 900,
      operation: {
        type: 'matchCorrectRuling',
        matchId: 'match-1',
        correctionId: 'correction-1',
        refereeId: 'referee-1',
        gameRecordId: 'game-1',
        replacementRulingRecordId: 'ruling-2',
        replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' } },
        reason: '裁判覆核',
      },
    })
})

test('拒絕空白理由、不完整 ruling、未知欄位及偽造身分或可信時間', () => {
  for (const body of [
    null,
    { ...validBody, reason: ' \t ' },
    { ...validBody, refereeId: 'attacker' },
    { ...validBody, callerUserId: 'attacker' },
    { ...validBody, trustedTimeMs: 0 },
    { ...validBody, serverActor: { type: 'referee' } },
    { ...validBody, binding: {} },
    { ...validBody, operation: { type: 'matchCorrectRuling' } },
    { ...validBody, replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' }, private: true } },
    { ...validBody, replacementRuling: { type: 'needsDecision', reason: 'invented' } },
    { ...validBody, expectedMatchRevision: Number.MAX_SAFE_INTEGER },
    { ...validBody, commandId: '' },
  ]) {
    expect(() => buildRulingCorrectionCommand(body)).toThrow()
  }
  expect(() => buildRulingCorrectionRequest('c', 'm', 'r', buildRulingCorrectionCommand(validBody), -1))
    .toThrow()
  const symbolic = { ...validBody }
  Object.defineProperty(symbolic, Symbol.for('unexpected-body-field'), { value: true })
  expect(() => buildRulingCorrectionCommand(symbolic)).toThrow()
})

test('接受每種既有 CompetitionRuling 的淺層型別，不替 core 判斷是否合法', () => {
  const rulings: CompetitionRuling[] = [
    { type: 'timeoutLoss', loser: 'team-a' },
    { type: 'doubleTimeoutLoss', teams: ['team-a', 'team-b'] },
    { type: 'gameAccepted', outcome: 'draw' },
    { type: 'gameAccepted', outcome: { winner: 'team-b' } },
    { type: 'eliminationDrawLoss', loser: 'team-a', reason: 'enteredByoyomiFirst' },
    { type: 'needsTiebreak', continuation: { teams: [
      { team: 'team-a', mainTimeRemainingMs: 0 }, { team: 'team-b', mainTimeRemainingMs: null },
    ] } },
    { type: 'needsDecision', reason: 'zeroTimeControl' },
  ]
  for (const replacementRuling of rulings) {
    expect(buildRulingCorrectionCommand({ ...validBody, replacementRuling }).replacementRuling)
      .toEqual(replacementRuling)
  }
})

test('ACK 僅投影核對過的更正回應白名單欄位', () => {
  const command = buildRulingCorrectionCommand(validBody)
  const request = buildRulingCorrectionRequest('comp-1', 'match-1', 'referee-1', command, 900)
  const value = {
    competitionId: 'comp-1', matchId: 'match-1', matchRevision: 5,
    commandId: 'command-1', idempotent: false, timeoutEvidence: null,
    nextClockDeadlineMs: null,
    matchProjection: { ok: true, operation: 'matchCorrectRuling', public: { match: { status: 'open' } } },
    clockProjection: null,
  }
  expect(projectRulingCorrectionAcknowledgement(value, request)).toEqual({
    competitionId: 'comp-1', matchId: 'match-1', matchRevision: 5,
    commandId: 'command-1', idempotent: false,
  })
  for (const malformed of [
    { ...value, private: { secret: true } },
    { ...value, serverState: {} },
    { ...value, matchRevision: 6 },
    { ...value, commandId: 'other' },
    { ...value, matchProjection: { ok: true, operation: 'matchSubmitGame', public: {} } },
    { ...value, matchProjection: { ok: true, operation: 'matchCorrectRuling', public: { private: {} } } },
    { ...value, timeoutEvidence: { type: 'timeout' } },
  ]) {
    expect(() => projectRulingCorrectionAcknowledgement(malformed, request)).toThrow()
  }
})
