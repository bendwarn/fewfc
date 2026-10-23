import { describe, expect, test } from 'bun:test'
import {
  CompetitionProtocolError,
  apiErrorStatus,
  buildCompetitionAnnouncement,
  buildRegisterTeamAction,
  createRegistrationCommandBody,
  createRegistrationViewBody,
  formatCompetitionDeadline,
  formatCompetitionRegistrationDeadline,
  registrationExclusionReasonLabel,
  isDefinitiveCompetitionCreateRejection,
  isRevisionConflict,
  localDateTimeToEpochMs,
  localMinuteDateTimeToEpochMs,
  parseCompetitionRegistrationResponse,
  parseCreatedCompetitionId,
  parseUserIdLines,
  selectActiveTeamForCaller,
} from './competition-registration'
import type { RuleModuleSpec } from '../types/fewfc'
import type { CompetitionAnnouncementStageDraft } from '../../shared/competition-announcement-plan'

const catalog: RuleModuleSpec[] = [
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'personal-deck', category: 'optional', defaultEnabled: false, dependencies: [] },
  { id: 'totem-formation', category: 'theme', defaultEnabled: true, dependencies: ['star'] },
  { id: 'echo', category: 'theme', defaultEnabled: true, dependencies: ['star'] },
]

const announcement = {
  organizer: 'organizer-a',
  ruleVersion: '5.17',
  ruleset: 'base',
  enabledRuleModules: ['star', 'personal-deck'],
  teamSize: 2,
  initialHp: 200,
  registrationMode: 'inviteOnly',
  personalDecks: 'required',
  registrationDeadline: 1_790_700_000_000,
  checkInDeadline: 1_790_760_000_000,
  plan: {
    initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
    swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
    rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
    exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
    stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
      swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' } }],
  },
}

function stageDrafts(): CompetitionAnnouncementStageDraft[] {
  return [{ stageId: 'stage-1', format: 'roundRobin', matchGames: '1', mainMinutes: '5', byoyomiSeconds: '30',
    swissRounds: '3', swissMatchPoints: { win: '3', draw: '1', loss: '0', bye: '3' },
    swissPolicy: {
      drawCredit: { numerator: '1', denominator: '2' },
      opponentWinRateFloorPercent: { mode: 'none', value: '' },
      opponentWinRateCapPercent: { mode: 'none', value: '' },
      excludeByesFromOpponentWinRate: 'no', tieMode: 'none', tieBreakers: [],
    }, sourceSlots: [] }]
}

const team = {
  teamId: 'team-a',
  captain: 'captain-a',
  members: ['captain-a', 'member-a'],
  rosterVersion: 3,
  confirmedMembers: ['captain-a'],
  status: 'registered',
  exclusionReasons: [],
}

function registrationResponse(options: { privateProjection?: unknown, extraEnvelope?: Record<string, unknown> } = {}) {
  const publicView = {
    announcement,
    lifecycle: 'open',
    revision: 2,
    teams: [team],
  }
  return {
    competitionId: 'competition-a',
    revision: 8,
    projection: {
      ok: true,
      operation: 'registrationView',
      public: publicView,
      ...(options.privateProjection === undefined ? {} : { private: options.privateProjection }),
    },
    ...options.extraEnvelope,
  }
}

describe('competition registration contracts', () => {
  test('converts browser-local date and time to exact Unix milliseconds', () => {
    const local = new Date(2026, 8, 30, 13, 14, 15, 6)
    expect(localDateTimeToEpochMs('2026-09-30T13:14:15.006')).toBe(local.getTime())
    expect(localDateTimeToEpochMs('2026-09-30T13:14')).toBe(new Date(2026, 8, 30, 13, 14).getTime())
  })

  test('rejects invalid local dates and impossible daylight-saving wall times', () => {
    expect(() => localDateTimeToEpochMs('2026-02-30T12:00')).toThrow()
    expect(() => localDateTimeToEpochMs('2026-09-30')).toThrow()
  })

  test('accepts only minute-precision input without rounding invalid dates', () => {
    const timestamp = localMinuteDateTimeToEpochMs('2026-09-30T13:14')
    expect(timestamp).toBe(new Date(2026, 8, 30, 13, 14).getTime())
    expect(new Date(timestamp).getSeconds()).toBe(0)
    expect(new Date(timestamp).getMilliseconds()).toBe(0)
    for (const invalid of [
      '2026-09-30T13:14:15.006', '2026-09-30T13:14:00', '2026-02-30T12:00',
      '2026-13-01T12:00', '2026-09-30T24:00', '2026-09-30T13:60',
      '2026-09-30', ' 2026-09-30T13:14 ', '',
    ]) {
      expect(() => localMinuteDateTimeToEpochMs(invalid)).toThrow()
    }
    // 通用 local date parser 仍獨立支援秒／毫秒；公告 builder 不使用此寬鬆格式。
    expect(localDateTimeToEpochMs('2026-09-30T13:14:15.006'))
      .toBe(new Date(2026, 8, 30, 13, 14, 15, 6).getTime())
  })

  test('formats competition deadlines to the minute with timezone without changing timestamp precision', () => {
    const timestamp = Date.UTC(2026, 8, 30, 13, 14, 15, 6)
    const formatted = formatCompetitionDeadline(timestamp, 'UTC')
    expect(formatted).toMatch(/2026\/09\/30.*01:14/u)
    expect(formatted).toContain('UTC')
    expect(formatted).not.toMatch(/01:14:15|006/u)
    expect(formatCompetitionDeadline(Date.UTC(2026, 8, 30, 13, 14, 59, 999), 'UTC'))
      .toBe(formatted)
    const registrationFormatted = formatCompetitionRegistrationDeadline(timestamp, 'UTC')
    expect(registrationFormatted).toBe(formatCompetitionDeadline(timestamp, 'UTC'))
    expect(formatCompetitionRegistrationDeadline(Date.UTC(2026, 8, 30, 13, 14, 59, 999), 'UTC'))
      .toBe(registrationFormatted)
    expect(formatCompetitionRegistrationDeadline(Number.NaN)).toBe('時間無法顯示')
    expect(formatCompetitionRegistrationDeadline(-1)).toBe('時間無法顯示')
    expect(formatCompetitionRegistrationDeadline(8_640_000_000_000_001)).toBe('時間無法顯示')
    expect(() => formatCompetitionDeadline(Number.NaN)).not.toThrow()
    expect(formatCompetitionDeadline(Number.NaN)).toBe('時間無法顯示')
  })

  test('uses known ruleset and version-filtered catalog modules for announcements', () => {
    const payload = buildCompetitionAnnouncement({
      ruleVersion: '5.17',
      enabledRuleModules: ['star', 'personal-deck'],
      teamSize: 2,
      registrationMode: 'inviteOnly',
      invitedCaptainsText: 'captain-a\n captain-b \n',
      registrationDeadlineLocal: '2026-09-30T13:14',
      checkInDeadlineLocal: '2026-09-30T14:00',
      stageDrafts: stageDrafts(),
    }, catalog)

    expect(payload.ruleset).toBe('base')
    expect(payload.enabledRuleModules).toEqual(['star', 'personal-deck'])
    expect(payload.personalDecks).toBe('required')
    expect(payload.registrationMode).toEqual({ inviteOnly: { invitedCaptains: ['captain-a', 'captain-b'] } })
    expect(payload.registrationDeadline).toBe(localMinuteDateTimeToEpochMs('2026-09-30T13:14'))
    expect(payload.checkInDeadline).toBe(localMinuteDateTimeToEpochMs('2026-09-30T14:00', '比賽開始時間'))
    expect(payload.plan).toMatchObject({
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        qualification: { type: 'allEligibleTeams' } }],
    })
  })

  test.each([
    { field: 'registrationDeadlineLocal' as const, label: '報名截止時間', validStart: '2026-09-30T14:00' },
    { field: 'checkInDeadlineLocal' as const, label: '比賽開始時間', validStart: '2026-09-30T13:14' },
  ])('announcement minute policy: rejects seconds and milliseconds for $label', ({ field, label, validStart }) => {
    for (const invalid of ['2026-09-30T13:14:00', '2026-09-30T13:14:00.000', '2026-09-30T13:14:15.006']) {
      const input = {
        ruleVersion: '5.17' as const,
        enabledRuleModules: ['star'],
        teamSize: 1,
        registrationMode: 'public' as const,
        invitedCaptainsText: '',
        registrationDeadlineLocal: field === 'registrationDeadlineLocal' ? invalid : '2026-09-30T13:14',
        checkInDeadlineLocal: field === 'checkInDeadlineLocal' ? invalid : validStart,
        stageDrafts: stageDrafts(),
      }
      expect(() => buildCompetitionAnnouncement(input, catalog))
        .toThrow(`${label}請以分鐘為單位輸入，不含秒或毫秒。`)
    }
  })

  test.each([
    { field: 'registrationDeadlineLocal' as const, label: '報名截止時間' },
    { field: 'checkInDeadlineLocal' as const, label: '比賽開始時間' },
  ])('announcement minute policy: rejects invalid dates in $label', ({ field }) => {
    const input = {
      ruleVersion: '5.17' as const,
      enabledRuleModules: ['star'],
      teamSize: 1,
      registrationMode: 'public' as const,
      invitedCaptainsText: '',
      registrationDeadlineLocal: field === 'registrationDeadlineLocal' ? '2026-02-30T12:00' : '2026-09-30T13:14',
      checkInDeadlineLocal: field === 'checkInDeadlineLocal' ? '2026-02-30T12:00' : '2026-09-30T14:00',
      stageDrafts: stageDrafts(),
    }
    expect(() => buildCompetitionAnnouncement(input, catalog)).toThrow()
  })

  test('announcement minute policy: preserves exact minute wire values, ordering, and frozen input', () => {
    const input = {
      ruleVersion: '5.17' as const,
      enabledRuleModules: ['star'],
      teamSize: 1,
      registrationMode: 'public' as const,
      invitedCaptainsText: '',
      registrationDeadlineLocal: '2026-09-30T13:14',
      checkInDeadlineLocal: '2026-09-30T14:00',
      stageDrafts: stageDrafts(),
    }
    Object.freeze(input)
    Object.freeze(input.enabledRuleModules)
    const before = JSON.stringify(input)
    const payload = buildCompetitionAnnouncement(input, catalog)

    expect(payload.registrationDeadline).toBe(new Date(2026, 8, 30, 13, 14).getTime())
    expect(payload.checkInDeadline).toBe(new Date(2026, 8, 30, 14, 0).getTime())
    expect(payload.registrationDeadline).toBeLessThan(payload.checkInDeadline)
    expect(JSON.stringify(input)).toBe(before)
  })

  test('announcement minute policy: keeps start-time terminology and allows equal deadlines', () => {
    const input = {
      ruleVersion: '5.17' as const,
      enabledRuleModules: ['star'],
      teamSize: 1,
      registrationMode: 'public' as const,
      invitedCaptainsText: '',
      registrationDeadlineLocal: '2026-09-30T15:00',
      checkInDeadlineLocal: '2026-09-30T14:00',
      stageDrafts: stageDrafts(),
    }
    expect(() => buildCompetitionAnnouncement(input, catalog))
      .toThrow('報名截止時間不可晚於比賽開始時間。')
    // 公告時間採分鐘精度，但相等仍合法。
    const equal = buildCompetitionAnnouncement({
      ...input,
      registrationDeadlineLocal: '2026-09-30T14:00',
      checkInDeadlineLocal: '2026-09-30T14:00',
    }, catalog)
    expect(equal.registrationDeadline).toBe(new Date(2026, 8, 30, 14, 0).getTime())
    expect(equal.checkInDeadline).toBe(equal.registrationDeadline)
  })

  test('rejects dependencies or modules excluded by the selected rule version', () => {
    expect(() => buildCompetitionAnnouncement({
      ruleVersion: '5.17',
      enabledRuleModules: ['echo'],
      teamSize: 1,
      registrationMode: 'public',
      invitedCaptainsText: '',
      registrationDeadlineLocal: '2026-09-30T13:14',
      checkInDeadlineLocal: '2026-09-30T14:00',
      stageDrafts: stageDrafts(),
    }, catalog)).toThrow()
  })

  test('parses invitee and team-member IDs from labelled line input', () => {
    expect(parseUserIdLines(' user-a\r\n\nuser-b ')).toEqual(['user-a', 'user-b'])
    expect(() => parseUserIdLines('user-a\n user-a ')).toThrow('使用者 ID 不可重複。')
    expect(buildRegisterTeamAction('team-b', 'captain-b', 'member-b', 2)).toEqual({
      type: 'registerTeam',
      teamId: 'team-b',
      members: ['captain-b', 'member-b'],
    })
    expect(() => buildRegisterTeamAction('team-b', 'captain-b', '', 2)).toThrow()
  })

  test('withdrawn public or private team does not block joining a new active team', () => {
    const withdrawn = { ...team, teamId: 'team-old', status: 'withdrawn' as const }
    const active = { ...team, teamId: 'team-new', status: 'registered' as const }
    const withdrawnPrivate = { ...withdrawn, ownDeckSubmitted: false }

    expect(selectActiveTeamForCaller([withdrawn, active], withdrawnPrivate, 'member-a')?.teamId).toBe('team-new')
    expect(selectActiveTeamForCaller([withdrawn], withdrawnPrivate, 'member-a')).toBeNull()
  })

  test('creates commands with secure UUIDs and keeps the caller identity out of request bodies', () => {
    const body = createRegistrationCommandBody(8, {
      type: 'confirmMembership', teamId: 'team-a', rosterVersion: 3,
    })
    expect(body.commandId).toMatch(/^[0-9a-f-]{36}$/iu)
    expect(body.expectedRevision).toBe(8)
    expect(body.operation).toEqual({
      type: 'registrationExecute',
      action: { type: 'confirmMembership', teamId: 'team-a', rosterVersion: 3 },
    })
    expect(body).not.toHaveProperty('callerUserId')
    expect(createRegistrationViewBody(1, () => 'view-id')).toEqual({
      commandId: 'view-id',
      expectedRevision: 1,
      operation: { type: 'registrationView' },
    })
  })

  test('retains the same semantic command identity and revision for a retry', () => {
    const action = { type: 'withdrawTeam', teamId: 'team-a' } as const
    const original = createRegistrationCommandBody(8, action, () => 'stable-command')
    const retry = original
    expect(retry.commandId).toBe('stable-command')
    expect(retry.expectedRevision).toBe(8)
    expect(retry.operation).toEqual(original.operation)
  })

  test('serializes a personal deck submission as an existing revisioned action', () => {
    const body = createRegistrationCommandBody(12, {
      type: 'submitDeck',
      teamId: 'team-a',
      rosterVersion: 4,
      name: '本人牌組',
      cards: ['metal-1', 'wood-1'],
    }, () => 'stable-command')
    expect(body).toEqual({
      commandId: 'stable-command',
      expectedRevision: 12,
      operation: {
        type: 'registrationExecute',
        action: {
          type: 'submitDeck',
          teamId: 'team-a',
          rosterVersion: 4,
          name: '本人牌組',
          cards: ['metal-1', 'wood-1'],
        },
      },
    })
    expect(JSON.stringify(body)).not.toMatch(/callerUserId|trustedTimeMs/)
  })

  test('uses the caller-specific projection but strips deck contents from UI data', () => {
    const publicView = {
      announcement,
      lifecycle: 'open',
      revision: 2,
      teams: [team],
    }
    const state = parseCompetitionRegistrationResponse(registrationResponse({
      privateProjection: {
        publicView,
        ownTeam: {
          teamId: team.teamId,
          captain: team.captain,
          members: team.members,
          rosterVersion: team.rosterVersion,
          confirmedMembers: team.confirmedMembers,
          status: team.status,
          ownDeck: { player: 'member-a', name: 'secret-deck', cards: ['secret-card'] },
        },
      },
    }), 'competition-a', undefined, 'member-a')

    expect(state.revision).toBe(8)
    expect(state.ownTeam?.teamId).toBe('team-a')
    expect(state.ownTeam?.ownDeckSubmitted).toBe(true)
    expect(JSON.stringify(state)).not.toMatch(/secret-deck|secret-card|"ownDeck"|serverState/)
  })

  test('reads the exact saved announcement plan and rejects an altered public policy', () => {
    const state = parseCompetitionRegistrationResponse(registrationResponse(), 'competition-a')
    expect(state.plan).toEqual(JSON.parse(JSON.stringify(announcement.plan)))

    const invalid = structuredClone(registrationResponse())
    invalid.projection.public.announcement.plan.rankingPolicy = 'matchPointsThenHealth'
    expect(() => parseCompetitionRegistrationResponse(invalid, 'competition-a'))
      .toThrow(CompetitionProtocolError)
  })

  test('rejects untrusted extra fields and private data for a different caller', () => {
    expect(() => parseCompetitionRegistrationResponse(
      registrationResponse({ extraEnvelope: { serverState: { registration: {} } } }),
      'competition-a',
    )).toThrow(CompetitionProtocolError)

    expect(() => parseCompetitionRegistrationResponse(registrationResponse({
      privateProjection: { publicView: { announcement }, ownTeam: team },
    }), 'competition-a', undefined, 'someone-else')).toThrow(CompetitionProtocolError)
  })

  test('accepts the exact competition creation acknowledgement', () => {
    const id = parseCreatedCompetitionId({ competitionId: 'competition-new' })
    expect(id).toBe('competition-new')
    expect(() => parseCreatedCompetitionId({ competitionId: 'competition-new', serverState: {} }))
      .toThrow(CompetitionProtocolError)
  })

  test('identifies 409 revision conflicts without trusting message strings', () => {
    expect(isRevisionConflict({ statusCode: 409 })).toBe(true)
    expect(isRevisionConflict({ status: 500, message: '409' })).toBe(false)
    expect(apiErrorStatus({ data: { status: 409 } })).toBeUndefined()
  })

  test('only permits create retries after responses known to reject before creation', () => {
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 400 })).toBe(true)
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 422 })).toBe(true)
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 501 })).toBe(true)
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 408 })).toBe(false)
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 409 })).toBe(false)
    expect(isDefinitiveCompetitionCreateRejection({ statusCode: 502 })).toBe(false)
    expect(isDefinitiveCompetitionCreateRejection(new Error('connection lost'))).toBe(false)
  })
})

describe('opening-freeze public registration schema', () => {
  function frozenResponse(reasons: unknown = ['missingCaptainConfirmation']) {
    const response = registrationResponse()
    return {
      ...response,
      projection: { ...response.projection, public: {
        ...response.projection.public,
        lifecycle: 'locked',
        teams: [{ ...team, status: 'notCheckedIn', exclusionReasons: reasons }],
      } },
    }
  }

  test('accepts the required seventh field and three exact camelCase reasons without missing-person or deck data', () => {
    const response = frozenResponse([
      'missingMemberConfirmations', 'missingRequiredDecks', 'missingCaptainConfirmation',
    ])
    const state = parseCompetitionRegistrationResponse(response, 'competition-a')
    expect(state.teams[0]?.exclusionReasons).toEqual([
      'missingMemberConfirmations', 'missingRequiredDecks', 'missingCaptainConfirmation',
    ])
    expect(state.teams[0]?.exclusionReasons).not.toBe(response.projection.public.teams[0]?.exclusionReasons)
    expect(Object.keys(state.teams[0] ?? {}).sort()).toEqual([
      'captain', 'confirmedMembers', 'exclusionReasons', 'members', 'rosterVersion', 'status', 'teamId',
    ])
    expect(registrationExclusionReasonLabel('missingMemberConfirmations')).toBe('成員確認未完成')
    expect(registrationExclusionReasonLabel('missingRequiredDecks')).toBe('必要牌組提交未完成')
    expect(registrationExclusionReasonLabel('missingCaptainConfirmation')).toBe('隊長資料完成確認未完成')
  })

  test('requires empty reasons for every non-excluded status, including withdrawn', () => {
    for (const status of ['registered', 'checkedIn', 'withdrawn']) {
      const response = registrationResponse()
      const withTeam = (exclusionReasons: unknown) => ({ ...response, projection: {
        ...response.projection, public: { ...response.projection.public, teams: [{ ...team, status, exclusionReasons }] },
      } })
      expect(parseCompetitionRegistrationResponse(withTeam([]), 'competition-a').teams[0]?.exclusionReasons).toEqual([])
      expect(() => parseCompetitionRegistrationResponse(withTeam(['missingCaptainConfirmation']), 'competition-a'))
        .toThrow(CompetitionProtocolError)
    }
  })

  test('rejects missing, snake-case, unknown, duplicate, empty or sparse reason contracts', () => {
    for (const reasons of [null, 'missingCaptainConfirmation', [],
      ['missing_captain_confirmation'], ['unknown'], ['missingCaptainConfirmation', 'missingCaptainConfirmation'],
      ['missingRequiredDecks'], new Array<string>(1), ['missingCaptainConfirmation', undefined]]) {
      expect(() => parseCompetitionRegistrationResponse(frozenResponse(reasons), 'competition-a'))
        .toThrow(CompetitionProtocolError)
    }
    const response = frozenResponse()
    const publicTeam = response.projection.public.teams[0]
    if (!publicTeam) throw new Error('需要固定隊伍 fixture')
    const { exclusionReasons, ...withoutReasons } = publicTeam
    for (const invalidTeam of [withoutReasons, { ...publicTeam, exclusionReasons: undefined },
      { ...withoutReasons, exclusion_reasons: exclusionReasons },
      { ...publicTeam, missingPlayers: ['private-person'] }]) {
      expect(() => parseCompetitionRegistrationResponse({ ...response, projection: {
        ...response.projection, public: { ...response.projection.public, teams: [invalidTeam] },
      } }, 'competition-a')).toThrow(CompetitionProtocolError)
    }
  })

  test('rejects accessors, symbolic extras and decorated arrays without invoking getters', () => {
    let getterCalls = 0
    const response = frozenResponse()
    const publicTeam = response.projection.public.teams[0]
    if (!publicTeam) throw new Error('需要固定隊伍 fixture')
    const teamGetter = { ...publicTeam }
    Object.defineProperty(teamGetter, 'exclusionReasons', { enumerable: true, get() {
      getterCalls += 1
      return ['missingCaptainConfirmation']
    } })
    const arrayGetter = ['missingCaptainConfirmation']
    Object.defineProperty(arrayGetter, '0', { enumerable: true, get() {
      getterCalls += 1
      return 'missingCaptainConfirmation'
    } })
    const arrayExtra = Object.assign(['missingCaptainConfirmation'], { private: 'secret' })
    const arraySymbol = Object.assign(['missingCaptainConfirmation'], { [Symbol('extra')]: true })
    for (const invalidTeam of [teamGetter, { ...publicTeam, [Symbol('extra')]: true },
      { ...publicTeam, exclusionReasons: arrayGetter }, { ...publicTeam, exclusionReasons: arrayExtra },
      { ...publicTeam, exclusionReasons: arraySymbol }]) {
      expect(() => parseCompetitionRegistrationResponse({ ...response, projection: {
        ...response.projection, public: { ...response.projection.public, teams: [invalidTeam] },
      } }, 'competition-a')).toThrow(CompetitionProtocolError)
    }
    expect(getterCalls).toBe(0)
  })

  test('rejects contradictory lifecycle or disabled-deck reason without deriving a second qualification model', () => {
    const response = frozenResponse(['missingRequiredDecks', 'missingCaptainConfirmation'])
    expect(() => parseCompetitionRegistrationResponse({ ...response, projection: { ...response.projection,
      public: { ...response.projection.public, lifecycle: 'open' },
    } }, 'competition-a')).toThrow(CompetitionProtocolError)
    expect(() => parseCompetitionRegistrationResponse({ ...response, projection: { ...response.projection,
      public: { ...response.projection.public, announcement: { ...announcement, personalDecks: 'disabled' } },
    } }, 'competition-a')).toThrow(CompetitionProtocolError)
    const disabled = frozenResponse(['missingMemberConfirmations', 'missingCaptainConfirmation'])
    expect(parseCompetitionRegistrationResponse({ ...disabled, projection: { ...disabled.projection,
      public: { ...disabled.projection.public, announcement: { ...announcement, personalDecks: 'disabled' } },
    } }, 'competition-a').teams[0]?.exclusionReasons).toEqual(['missingMemberConfirmations', 'missingCaptainConfirmation'])
  })

  test('checks envelope and nested descriptors before access and rejects sparse teams without getter execution', () => {
    let getterCalls = 0
    const response = frozenResponse()
    const invalidEnvelope = { ...response }
    Object.defineProperty(invalidEnvelope, 'competitionId', { enumerable: true, get() {
      getterCalls += 1
      return 'competition-a'
    } })
    const invalidProjection = { ...response.projection }
    Object.defineProperty(invalidProjection, 'ok', { enumerable: true, get() {
      getterCalls += 1
      return true
    } })
    const invalidPublic = { ...response.projection.public }
    Object.defineProperty(invalidPublic, 'teams', { enumerable: true, get() {
      getterCalls += 1
      return response.projection.public.teams
    } })
    for (const invalid of [invalidEnvelope, { ...response, projection: invalidProjection },
      { ...response, projection: { ...response.projection, public: invalidPublic } },
      { ...response, projection: { ...response.projection, public: {
        ...response.projection.public, teams: new Array<unknown>(1),
      } } }]) {
      expect(() => parseCompetitionRegistrationResponse(invalid, 'competition-a')).toThrow(CompetitionProtocolError)
    }
    expect(getterCalls).toBe(0)
  })

  test('restores own-team reasons only from matching validated public teams while private ownTeam stays seven fields', () => {
    const response = frozenResponse(['missingMemberConfirmations', 'missingCaptainConfirmation'])
    const publicView = response.projection.public
    const ownTeam = { teamId: team.teamId, captain: team.captain, members: team.members,
      rosterVersion: team.rosterVersion, confirmedMembers: team.confirmedMembers, status: 'notCheckedIn',
      ownDeck: { player: 'member-a', name: 'secret-deck', cards: ['secret-card'] } }
    const withPrivate = (privateTeam: unknown, privatePublicView: unknown = publicView) => ({ ...response, projection: {
      ...response.projection, private: { publicView: privatePublicView, ownTeam: privateTeam },
    } })
    const state = parseCompetitionRegistrationResponse(withPrivate(ownTeam), 'competition-a', undefined, 'member-a')
    expect(Object.keys(ownTeam)).toHaveLength(7)
    expect(state.ownTeam?.exclusionReasons).toEqual(['missingMemberConfirmations', 'missingCaptainConfirmation'])
    expect(state.ownTeam?.ownDeckSubmitted).toBe(true)
    expect(JSON.stringify(state)).not.toMatch(/secret-deck|secret-card|"ownDeck"/u)
    for (const change of [{ teamId: 'other-team' }, { captain: 'member-a' }, { members: ['member-a', 'captain-a'] },
      { rosterVersion: 4 }, { confirmedMembers: ['member-a'] }, { status: 'registered' },
      { exclusionReasons: ['missingCaptainConfirmation'] }]) {
      expect(() => parseCompetitionRegistrationResponse(withPrivate({ ...ownTeam, ...change }), 'competition-a', undefined, 'member-a'))
        .toThrow(CompetitionProtocolError)
    }
    expect(() => parseCompetitionRegistrationResponse(withPrivate(ownTeam, {
      ...publicView, teams: [{ ...team, status: 'notCheckedIn', exclusionReasons: ['missingCaptainConfirmation'] }],
    }), 'competition-a', undefined, 'member-a')).toThrow(CompetitionProtocolError)
    expect(() => parseCompetitionRegistrationResponse(withPrivate({ ...ownTeam, ownDeck: {
      ...ownTeam.ownDeck, exclusionReasons: ['missingRequiredDecks'],
    } }), 'competition-a', undefined, 'member-a')).toThrow(CompetitionProtocolError)
    let getterCalls = 0
    const privateGetter = { ...ownTeam }
    Object.defineProperty(privateGetter, 'status', { enumerable: true, get() {
      getterCalls += 1
      return 'notCheckedIn'
    } })
    expect(() => parseCompetitionRegistrationResponse(withPrivate(privateGetter), 'competition-a', undefined, 'member-a'))
      .toThrow(CompetitionProtocolError)
    expect(getterCalls).toBe(0)
  })
})
