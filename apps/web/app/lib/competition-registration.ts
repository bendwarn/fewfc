import type { RuleModuleSpec } from '../types/fewfc'
import { createRuleModulePolicy } from '#shared/utils/rule-modules'
import { modulesForVersion } from '#shared/utils/rule-versions'
import { buildCompetitionAnnouncementPlan, parseCompetitionAnnouncementPlan, type CompetitionAnnouncementStageDraft } from '#shared/competition-announcement-plan'
import type { CompetitionAnnouncementPlan } from '#shared/competition'

export type CompetitionRuleVersion = '5.16' | '5.17'
export type CompetitionTeamStatus = 'registered' | 'checkedIn' | 'withdrawn' | 'notCheckedIn'
export type CompetitionRegistrationLifecycle = 'draft' | 'open' | 'locked'
export type RegistrationExclusionReason =
  | 'missingMemberConfirmations'
  | 'missingRequiredDecks'
  | 'missingCaptainConfirmation'

export interface CompetitionPublicTeam {
  teamId: string
  captain: string
  members: string[]
  rosterVersion: number
  confirmedMembers: string[]
  status: CompetitionTeamStatus
  exclusionReasons: RegistrationExclusionReason[]
}

export interface CompetitionOwnTeam extends CompetitionPublicTeam {
  ownDeckSubmitted: boolean
}

export interface CompetitionRegistrationState {
  competitionId: string
  revision: number
  organizer: string
  ruleVersion: CompetitionRuleVersion
  ruleset: string
  enabledRuleModules: string[]
  teamSize: number
  initialHp: number
  registrationMode: 'public' | 'inviteOnly'
  personalDecks: 'disabled' | 'required'
  registrationDeadline: number
  checkInDeadline: number
  plan: CompetitionAnnouncementPlan
  lifecycle: CompetitionRegistrationLifecycle
  teams: CompetitionPublicTeam[]
  ownTeam: CompetitionOwnTeam | null
}

export type RegistrationMutationAction =
  | { type: 'open' }
  | { type: 'registerTeam'; teamId: string; members: string[] }
  | { type: 'confirmMembership'; teamId: string; rosterVersion: number }
  | { type: 'checkInTeam'; teamId: string; rosterVersion: number }
  | { type: 'submitDeck'; teamId: string; rosterVersion: number; name: string; cards: string[] }
  | { type: 'withdrawTeam'; teamId: string }
  | { type: 'lock' }

export interface RegistrationCommandBody {
  commandId: string
  expectedRevision: number
  operation: { type: 'registrationExecute'; action: RegistrationMutationAction }
}

export interface RegistrationViewBody {
  commandId: string
  expectedRevision: number
  operation: { type: 'registrationView' }
}

export interface CompetitionAnnouncementFormInput {
  ruleVersion: CompetitionRuleVersion
  enabledRuleModules: string[]
  teamSize: number
  registrationMode: 'public' | 'inviteOnly'
  invitedCaptainsText: string
  registrationDeadlineLocal: string
  checkInDeadlineLocal: string
  stageDrafts: readonly CompetitionAnnouncementStageDraft[]
}

export interface CompetitionAnnouncementBody {
  ruleVersion: CompetitionRuleVersion
  ruleset: 'base'
  enabledRuleModules: string[]
  teamSize: number
  registrationMode: 'public' | { inviteOnly: { invitedCaptains: string[] } }
  personalDecks: 'disabled' | 'required'
  registrationDeadline: number
  checkInDeadline: number
  plan: CompetitionAnnouncementPlan
}

export class CompetitionProtocolError extends Error {
  constructor() {
    super('賽事資料格式無法辨識。')
    this.name = 'CompetitionProtocolError'
  }
}

export function parseCompetitionRegistrationResponse(
  input: unknown,
  expectedCompetitionId: string,
  expectedCommandId?: string,
  currentUserId?: string,
): CompetitionRegistrationState {
  const envelope = asRecord(input)
  if (!envelope || !isExactShape(envelope, [
    ['competitionId', 'revision', 'projection'],
    ['competitionId', 'revision', 'commandId', 'idempotent', 'projection'],
  ]) || envelope.competitionId !== expectedCompetitionId
    || !isSafeInteger(envelope.revision) || envelope.revision < 1) {
    throw new CompetitionProtocolError()
  }
  if ('commandId' in envelope
    && (typeof envelope.commandId !== 'string' || !envelope.commandId
      || typeof envelope.idempotent !== 'boolean'
      || (expectedCommandId !== undefined && envelope.commandId !== expectedCommandId))) {
    throw new CompetitionProtocolError()
  }

  const projection = asRecord(envelope.projection)
  if (!projection || !isExactShape(projection, [
    ['ok', 'operation', 'public'],
    ['ok', 'operation', 'public', 'private'],
  ]) || projection.ok !== true || typeof projection.operation !== 'string'
    || !['registrationCreate', 'registrationView', 'registrationExecute'].includes(projection.operation)) {
    throw new CompetitionProtocolError()
  }

  const publicView = parsePublicRegistrationView(projection.public)
  let ownTeam: CompetitionOwnTeam | null = null
  if ('private' in projection) {
    if (!currentUserId) throw new CompetitionProtocolError()
    ownTeam = parsePrivateRegistrationView(projection.private, currentUserId, publicView)
  }

  return {
    competitionId: expectedCompetitionId,
    revision: envelope.revision,
    organizer: publicView.announcement.organizer,
    ruleVersion: publicView.announcement.ruleVersion,
    ruleset: publicView.announcement.ruleset,
    enabledRuleModules: publicView.announcement.enabledRuleModules,
    teamSize: publicView.announcement.teamSize,
    initialHp: publicView.announcement.initialHp,
    registrationMode: publicView.announcement.registrationMode,
    personalDecks: publicView.announcement.personalDecks,
    registrationDeadline: publicView.announcement.registrationDeadline,
    checkInDeadline: publicView.announcement.checkInDeadline,
    plan: publicView.announcement.plan,
    lifecycle: publicView.lifecycle,
    teams: publicView.teams,
    ownTeam,
  }
}

export function parseCreatedCompetitionId(input: unknown): string {
  const envelope = asRecord(input)
  if (!envelope
    || !isExactKeys(envelope, ['competitionId'])
    || typeof envelope.competitionId !== 'string' || !envelope.competitionId.trim()) {
    throw new CompetitionProtocolError()
  }
  return envelope.competitionId
}

export function createRegistrationViewBody(
  expectedRevision: number,
  createId: () => string = () => crypto.randomUUID(),
): RegistrationViewBody {
  if (!isSafeInteger(expectedRevision) || expectedRevision < 1) throw new CompetitionProtocolError()
  const commandId = createId()
  if (!commandId.trim()) throw new CompetitionProtocolError()
  return { commandId, expectedRevision, operation: { type: 'registrationView' } }
}

export function createRegistrationCommandBody(
  expectedRevision: number,
  action: RegistrationMutationAction,
  createId: () => string = () => crypto.randomUUID(),
): RegistrationCommandBody {
  if (!isSafeInteger(expectedRevision) || expectedRevision < 1) throw new CompetitionProtocolError()
  const commandId = createId()
  if (!commandId.trim()) throw new CompetitionProtocolError()
  return {
    commandId,
    expectedRevision,
    operation: { type: 'registrationExecute', action },
  }
}

export function buildCompetitionAnnouncement(
  input: CompetitionAnnouncementFormInput,
  catalogModules: readonly RuleModuleSpec[],
): CompetitionAnnouncementBody {
  if (!isRuleVersion(input.ruleVersion)
    || !Number.isSafeInteger(input.teamSize) || input.teamSize < 1) {
    throw new Error('請確認規則版本與參賽型態。')
  }

  const availableModules = modulesForVersion(catalogModules, input.ruleVersion)
  const policy = createRuleModulePolicy(availableModules)
  const allowedIds = new Set(availableModules.map(module => module.id))
  const enabled = input.enabledRuleModules
  if (new Set(enabled).size !== enabled.length
    || enabled.some(id => !allowedIds.has(id))
    || !policy.hasValidDependencies(enabled)) {
    throw new Error('規則模組設定不完整，請重新確認選項。')
  }

  const registrationDeadline = localMinuteDateTimeToEpochMs(input.registrationDeadlineLocal, '報名截止時間')
  const checkInDeadline = localMinuteDateTimeToEpochMs(input.checkInDeadlineLocal, '比賽開始時間')
  if (registrationDeadline > checkInDeadline) {
    throw new Error('報名截止時間不可晚於比賽開始時間。')
  }

  let registrationMode: CompetitionAnnouncementBody['registrationMode'] = 'public'
  if (input.registrationMode === 'inviteOnly') {
    const invitedCaptains = parseUserIdLines(input.invitedCaptainsText)
    if (!invitedCaptains.length) throw new Error('邀請制至少需要一位受邀隊長。')
    registrationMode = { inviteOnly: { invitedCaptains } }
  }

  const plan = buildCompetitionAnnouncementPlan(input.stageDrafts)

  return {
    ruleVersion: input.ruleVersion,
    ruleset: 'base',
    enabledRuleModules: [...enabled],
    teamSize: input.teamSize,
    registrationMode,
    personalDecks: enabled.includes('personal-deck') ? 'required' : 'disabled',
    registrationDeadline,
    checkInDeadline,
    plan,
  }
}

export function parseUserIdLines(value: string): string[] {
  const ids = value.split(/\r?\n/u).map(id => id.trim()).filter(Boolean)
  if (new Set(ids).size !== ids.length) throw new Error('使用者 ID 不可重複。')
  return ids
}

export function buildRegisterTeamAction(
  teamId: string,
  currentUserId: string,
  otherMemberIdsText: string,
  teamSize: number,
): RegistrationMutationAction {
  const captain = currentUserId.trim()
  const otherMembers = parseUserIdLines(otherMemberIdsText)
  if (!teamId.trim() || !captain || !Number.isSafeInteger(teamSize) || teamSize < 1
    || otherMembers.length !== teamSize - 1
    || otherMembers.includes(captain)) {
    throw new Error(`每隊須有 ${teamSize} 位不同成員，並包含目前登入者。`)
  }
  return { type: 'registerTeam', teamId, members: [captain, ...otherMembers] }
}

export function localDateTimeToEpochMs(value: string): number {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/u.exec(value)
  if (!match) throw new Error('請輸入有效的日期與時間。')
  const [, yearText, monthText, dayText, hourText, minuteText, secondText = '0', fractionText = '0'] = match
  const year = Number(yearText)
  const month = Number(monthText)
  const day = Number(dayText)
  const hour = Number(hourText)
  const minute = Number(minuteText)
  const second = Number(secondText)
  const millisecond = Number(fractionText.padEnd(3, '0'))
  if (year < 1 || month < 1 || month > 12 || day < 1 || day > 31
    || hour > 23 || minute > 59 || second > 59) {
    throw new Error('請輸入有效的日期與時間。')
  }

  const date = new Date(0)
  date.setFullYear(year, month - 1, day)
  date.setHours(hour, minute, second, millisecond)
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day
    || date.getHours() !== hour || date.getMinutes() !== minute
    || date.getSeconds() !== second || date.getMilliseconds() !== millisecond) {
    throw new Error('所選日期時間在本地時區無效，請改選其他時間。')
  }
  const timestamp = date.getTime()
  if (!Number.isSafeInteger(timestamp) || timestamp < 0) throw new Error('日期時間超出可用範圍。')
  return timestamp
}

/** 公告表單的 F／T 只接受 HH:mm；不捨棄秒／毫秒，也不把無效日期進位成另一日。 */
export function localMinuteDateTimeToEpochMs(
  value: string,
  fieldLabel: '報名截止時間' | '比賽開始時間' = '報名截止時間',
): number {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/u.test(value)) {
    throw new Error(`${fieldLabel}請以分鐘為單位輸入，不含秒或毫秒。`)
  }
  return localDateTimeToEpochMs(value)
}

export function apiErrorStatus(error: unknown): number | undefined {
  const record = asRecord(error)
  const status = record?.statusCode ?? record?.status
  return typeof status === 'number' && Number.isSafeInteger(status) ? status : undefined
}

export function isRevisionConflict(error: unknown): boolean {
  return apiErrorStatus(error) === 409
}

export function isDefinitiveCompetitionCreateRejection(error: unknown): boolean {
  const status = apiErrorStatus(error)
  return status !== undefined && [400, 401, 403, 422, 501].includes(status)
}

export function selectActiveTeamForCaller(
  teams: readonly CompetitionPublicTeam[],
  ownTeam: CompetitionOwnTeam | null,
  currentUserId: string,
): CompetitionPublicTeam | CompetitionOwnTeam | null {
  if (!currentUserId) return null
  if (ownTeam?.status !== 'withdrawn' && ownTeam?.members.includes(currentUserId)) return ownTeam
  return teams.find(team => team.status !== 'withdrawn' && team.members.includes(currentUserId)) ?? null
}

export function formatCompetitionDeadline(timestamp: number, timeZone?: string): string {
  return formatDeadline(timestamp, timeZone)
}

/** 公告顯示只到分鐘；保留原 timestamp，包含既有期限的秒／毫秒精度。 */
export function formatCompetitionRegistrationDeadline(timestamp: number, timeZone?: string): string {
  return formatCompetitionDeadline(timestamp, timeZone)
}

export function registrationExclusionReasonLabel(reason: RegistrationExclusionReason): string {
  return {
    missingMemberConfirmations: '成員確認未完成',
    missingRequiredDecks: '必要牌組提交未完成',
    missingCaptainConfirmation: '隊長資料完成確認未完成',
  }[reason]
}

function formatDeadline(timestamp: number, timeZone?: string): string {
  if (!Number.isSafeInteger(timestamp) || timestamp < 0) return '時間無法顯示'
  try {
    const options: Intl.DateTimeFormatOptions = {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      timeZoneName: 'short',
    }
    if (timeZone !== undefined) options.timeZone = timeZone
    return new Intl.DateTimeFormat('zh-TW', options).format(timestamp)
  } catch {
    return '時間無法顯示'
  }
}

function parsePublicRegistrationView(input: unknown): ParsedPublicRegistrationView {
  const view = asRecord(input)
  if (!view || !isExactKeys(view, ['announcement', 'lifecycle', 'revision', 'teams'])
    || !isSafeInteger(view.revision) || view.revision < 0
    || !isLifecycle(view.lifecycle) || !isDenseDataArray(view.teams)) {
    throw new CompetitionProtocolError()
  }
  const announcement = asRecord(view.announcement)
  if (!announcement || !isExactKeys(announcement, [
    'organizer', 'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize', 'initialHp',
    'registrationMode', 'personalDecks', 'registrationDeadline', 'checkInDeadline', 'plan',
  ]) || typeof announcement.organizer !== 'string' || !announcement.organizer
    || !isRuleVersion(announcement.ruleVersion)
    || typeof announcement.ruleset !== 'string' || !announcement.ruleset
    || !isStringArray(announcement.enabledRuleModules)
    || !isSafeInteger(announcement.teamSize) || announcement.teamSize < 1
    || !isSafeInteger(announcement.initialHp) || announcement.initialHp < 1
    || !isRegistrationMode(announcement.registrationMode)
    || (announcement.personalDecks !== 'disabled' && announcement.personalDecks !== 'required')
    || !isSafeInteger(announcement.registrationDeadline) || announcement.registrationDeadline < 0
    || !isSafeInteger(announcement.checkInDeadline) || announcement.checkInDeadline < 0
    || announcement.registrationDeadline > announcement.checkInDeadline
    || announcement.registrationDeadline % 60_000 !== 0 || announcement.checkInDeadline % 60_000 !== 0) {
    throw new CompetitionProtocolError()
  }
  const plan = parseCompetitionAnnouncementPlan(announcement.plan)
  if (!plan) throw new CompetitionProtocolError()

  const teams = view.teams.map(parsePublicTeam)
  if (new Set(teams.map(team => team.teamId)).size !== teams.length) throw new CompetitionProtocolError()
  for (const team of teams) {
    if (team.members.length !== announcement.teamSize || !team.members.includes(team.captain)) {
      throw new CompetitionProtocolError()
    }
    if (team.status === 'notCheckedIn' && (view.lifecycle !== 'locked'
      || (announcement.personalDecks === 'disabled' && team.exclusionReasons.includes('missingRequiredDecks')))) {
      throw new CompetitionProtocolError()
    }
  }

  return {
    announcement: {
      organizer: announcement.organizer,
      ruleVersion: announcement.ruleVersion,
      ruleset: announcement.ruleset,
      enabledRuleModules: [...announcement.enabledRuleModules],
      teamSize: announcement.teamSize,
      initialHp: announcement.initialHp,
      registrationMode: announcement.registrationMode,
      personalDecks: announcement.personalDecks,
      registrationDeadline: announcement.registrationDeadline,
      checkInDeadline: announcement.checkInDeadline,
      plan,
    },
    lifecycle: view.lifecycle,
    revision: view.revision,
    teams,
  }
}

function parsePublicTeam(input: unknown): CompetitionPublicTeam {
  const team = asRecord(input)
  if (!team || !isExactKeys(team, [
    'teamId', 'captain', 'members', 'rosterVersion', 'confirmedMembers', 'status', 'exclusionReasons',
  ])) {
    throw new CompetitionProtocolError()
  }

  const members = team.members
  const confirmedMembers = team.confirmedMembers
  const exclusionReasons = team.exclusionReasons
  if (typeof team.teamId !== 'string' || !team.teamId
    || typeof team.captain !== 'string' || !team.captain
    || !isStringArray(members) || !isSafeInteger(team.rosterVersion) || team.rosterVersion < 1
    || !isStringArray(confirmedMembers) || !isTeamStatus(team.status)
    || !isDenseDataArray(exclusionReasons) || !exclusionReasons.every(isExclusionReason)) {
    throw new CompetitionProtocolError()
  }

  if (new Set(members).size !== members.length
    || new Set(confirmedMembers).size !== confirmedMembers.length
    || confirmedMembers.some(member => !members.includes(member))
    || new Set(exclusionReasons).size !== exclusionReasons.length
    || (team.status === 'notCheckedIn'
      ? !exclusionReasons.includes('missingCaptainConfirmation')
      : exclusionReasons.length !== 0)) {
    throw new CompetitionProtocolError()
  }

  return {
    teamId: team.teamId,
    captain: team.captain,
    members: [...members],
    rosterVersion: team.rosterVersion,
    confirmedMembers: [...confirmedMembers],
    status: team.status,
    exclusionReasons: [...exclusionReasons],
  }
}

function parsePrivateRegistrationView(
  input: unknown,
  currentUserId: string,
  publicView: ParsedPublicRegistrationView,
): CompetitionOwnTeam | null {
  const view = asRecord(input)
  if (!view || !isExactKeys(view, ['publicView', 'ownTeam'])) throw new CompetitionProtocolError()
  const privatePublicView = parsePublicRegistrationView(view.publicView)
  if (JSON.stringify(privatePublicView) !== JSON.stringify(publicView)) throw new CompetitionProtocolError()
  if (view.ownTeam === null) return null

  const ownTeamRecord = asRecord(view.ownTeam)
  if (!ownTeamRecord || !isExactKeys(ownTeamRecord, [
    'teamId', 'captain', 'members', 'rosterVersion', 'confirmedMembers', 'status', 'ownDeck',
  ])) throw new CompetitionProtocolError()
  const matchingTeam = publicView.teams.find(team => team.teamId === ownTeamRecord.teamId)
  if (!matchingTeam) throw new CompetitionProtocolError()
  // 私人 DTO 沒有理由欄；只補回已驗公開鏡像，仍逐一核對原六欄與本人身份。
  const publicTeam = parsePublicTeam({
    teamId: ownTeamRecord.teamId,
    captain: ownTeamRecord.captain,
    members: ownTeamRecord.members,
    rosterVersion: ownTeamRecord.rosterVersion,
    confirmedMembers: ownTeamRecord.confirmedMembers,
    status: ownTeamRecord.status,
    exclusionReasons: matchingTeam.exclusionReasons,
  })
  if (!publicTeam.members.includes(currentUserId)) throw new CompetitionProtocolError()
  if (JSON.stringify(matchingTeam) !== JSON.stringify(publicTeam)) {
    throw new CompetitionProtocolError()
  }

  let ownDeckSubmitted = false
  if (ownTeamRecord.ownDeck !== null) {
    const ownDeck = asRecord(ownTeamRecord.ownDeck)
    if (!ownDeck || !isExactKeys(ownDeck, ['player', 'name', 'cards'])
      || ownDeck.player !== currentUserId || typeof ownDeck.name !== 'string' || !isStringArray(ownDeck.cards)) {
      throw new CompetitionProtocolError()
    }
    ownDeckSubmitted = true
  }
  return { ...publicTeam, ownDeckSubmitted }
}

interface ParsedPublicRegistrationView {
  announcement: {
    organizer: string
    ruleVersion: CompetitionRuleVersion
    ruleset: string
    enabledRuleModules: string[]
    teamSize: number
    initialHp: number
    registrationMode: 'public' | 'inviteOnly'
    personalDecks: 'disabled' | 'required'
    registrationDeadline: number
    checkInDeadline: number
    plan: CompetitionAnnouncementPlan
  }
  lifecycle: CompetitionRegistrationLifecycle
  revision: number
  teams: CompetitionPublicTeam[]
}

function isRegistrationMode(value: unknown): value is 'public' | 'inviteOnly' {
  return value === 'public' || value === 'inviteOnly'
}

function isLifecycle(value: unknown): value is CompetitionRegistrationLifecycle {
  return value === 'draft' || value === 'open' || value === 'locked'
}

function isTeamStatus(value: unknown): value is CompetitionTeamStatus {
  return value === 'registered' || value === 'checkedIn'
    || value === 'withdrawn' || value === 'notCheckedIn'
}

function isRuleVersion(value: unknown): value is CompetitionRuleVersion {
  return value === '5.16' || value === '5.17'
}

function isStringArray(value: unknown): value is string[] {
  return isDenseDataArray(value) && value.every(item => typeof item === 'string')
}

function isExclusionReason(value: unknown): value is RegistrationExclusionReason {
  return value === 'missingMemberConfirmations' || value === 'missingRequiredDecks'
    || value === 'missingCaptainConfirmation'
}

function isDenseDataArray(value: unknown): value is unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) return false
  const keys = Reflect.ownKeys(value)
  if (keys.length !== value.length + 1) return false
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return false
  }
  return true
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value)
}

function isExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return false
  const actual = Reflect.ownKeys(value)
  return actual.length === expected.length && expected.every(key => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    return descriptor !== undefined && descriptor.enumerable && 'value' in descriptor
  })
}

function isExactShape(value: Record<string, unknown>, shapes: readonly (readonly string[])[]): boolean {
  return shapes.some(shape => isExactKeys(value, shape))
}
