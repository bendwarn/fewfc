import { createError, type H3Event } from 'h3'
import { isExactCompetitionJson, type JsonObject, type RegistrationMode } from '../../shared/competition'
import { parseCompetitionAnnouncementPlan } from '../../shared/competition-announcement-plan'
import type {
  CompetitionCreateAcknowledgement,
  CompetitionAnnouncementDraft,
  CompetitionRoomInitializeFromRulesRequest,
  CompetitionRoomInitializeValue,
} from '../../shared/competition-room'
import { workerEnv } from './worker-env'

const ANNOUNCEMENT_KEYS = [
  'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize', 'registrationMode',
  'personalDecks', 'registrationDeadline', 'checkInDeadline', 'plan',
]
const ERRORS = {
  invalidRequest: { statusCode: 400, statusMessage: 'Bad Request', message: '賽事要求格式不正確。' },
  forbidden: { statusCode: 403, statusMessage: 'Forbidden', message: '你沒有建立賽事的權限。' },
  competitionIdMismatch: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這個賽事。' },
  alreadyInitialized: { statusCode: 409, statusMessage: 'Conflict', message: '賽事已經建立。' },
  engineRejected: { statusCode: 422, statusMessage: 'Unprocessable Entity', message: '賽事設定無法使用。' },
  engineProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事服務回應格式不正確。' },
  competitionRoomUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', message: '賽事服務暫時無法使用。' },
  serverSourceUnavailable: { statusCode: 500, statusMessage: 'Internal Server Error', message: '伺服器建立資料暫時無法使用。' },
}

export function competitionCreateFromRulesError(code: keyof typeof ERRORS) {
  const error = ERRORS[code]
  // 固定 ASCII reason；中文訊息與機器碼均採白名單，不反射引擎／binding 文字。
  return createError({ ...error, data: { code, message: error.message } })
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function exact(value: Record<string, unknown>, keys: readonly string[], root = false): boolean {
  const actual = Reflect.ownKeys(value).filter(key => !(root && key === Symbol.dispose))
  return actual.length === keys.length && keys.every(key => {
    const field = Object.getOwnPropertyDescriptor(value, key)
    return field !== undefined && field.enumerable === true && Object.hasOwn(field, 'value')
  })
}

function sameAnnouncementPlan(actual: unknown, expected: unknown): boolean {
  const parsedActual = parseCompetitionAnnouncementPlan(actual)
  const parsedExpected = parseCompetitionAnnouncementPlan(expected)
  // exact parser 先拒絕未知或不完整欄位，再以固定欄位順序比較 JSON；物件鍵順序不代表內容差異。
  return parsedActual !== null && parsedExpected !== null
    && JSON.stringify(parsedActual) === JSON.stringify(parsedExpected)
}

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function timestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function uniqueStrings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(nonEmpty) && new Set(value).size === value.length
}

function registrationMode(value: unknown): RegistrationMode {
  if (value === 'public') return value
  if (record(value) && exact(value, ['inviteOnly']) && record(value.inviteOnly)
    && exact(value.inviteOnly, ['invitedCaptains']) && uniqueStrings(value.inviteOnly.invitedCaptains)
    && value.inviteOnly.invitedCaptains.length > 0) {
    return { inviteOnly: { invitedCaptains: [...value.inviteOnly.invitedCaptains] } }
  }
  throw competitionCreateFromRulesError('invalidRequest')
}

function announcementDraft(input: unknown): CompetitionAnnouncementDraft {
  if (!isExactCompetitionJson(input) || !record(input) || !exact(input, ANNOUNCEMENT_KEYS)) {
    throw competitionCreateFromRulesError('invalidRequest')
  }
  const plan = parseCompetitionAnnouncementPlan(input.plan)
  if ((input.ruleVersion !== '5.16' && input.ruleVersion !== '5.17') || !nonEmpty(input.ruleset)
    || !uniqueStrings(input.enabledRuleModules) || (input.teamSize !== 1 && input.teamSize !== 2)
    || (input.personalDecks !== 'disabled' && input.personalDecks !== 'required')
    || input.personalDecks !== (input.enabledRuleModules.includes('personal-deck') ? 'required' : 'disabled')
    || !timestamp(input.registrationDeadline) || input.registrationDeadline % 60_000 !== 0
    || !timestamp(input.checkInDeadline) || input.checkInDeadline % 60_000 !== 0
    || input.registrationDeadline > input.checkInDeadline || !plan) {
    throw competitionCreateFromRulesError('invalidRequest')
  }
  return {
    ruleVersion: input.ruleVersion, ruleset: input.ruleset, enabledRuleModules: [...input.enabledRuleModules],
    teamSize: input.teamSize, registrationMode: registrationMode(input.registrationMode),
    personalDecks: input.personalDecks, registrationDeadline: input.registrationDeadline,
    checkInDeadline: input.checkInDeadline,
    plan,
  }
}

function publicValue(input: unknown, request: CompetitionRoomInitializeFromRulesRequest): CompetitionRoomInitializeValue {
  if (!isExactCompetitionJson(input) || !record(input)
    || !exact(input, ['competitionId', 'organizerUserId', 'revision', 'projection'])
    || input.competitionId !== request.competitionId || input.organizerUserId !== request.callerUserId
    || input.revision !== 1 || !record(input.projection)
    || !exact(input.projection, ['ok', 'operation', 'public']) || input.projection.ok !== true
    || input.projection.operation !== 'registrationCreate' || !record(input.projection.public)) {
    throw competitionCreateFromRulesError('engineProtocolError')
  }
  const view = input.projection.public
  const announcement = view.announcement
  const expected = request.announcement
  const mode = expected.registrationMode === 'public' ? 'public' : 'inviteOnly'
  if (!exact(view, ['announcement', 'lifecycle', 'revision', 'teams'])
    || view.lifecycle !== 'draft' || view.revision !== 0 || !Array.isArray(view.teams) || view.teams.length !== 0
    || !record(announcement) || !exact(announcement, [...ANNOUNCEMENT_KEYS, 'organizer', 'initialHp'])
    || announcement.organizer !== request.callerUserId || announcement.ruleVersion !== expected.ruleVersion
    || announcement.ruleset !== expected.ruleset || announcement.teamSize !== expected.teamSize
    || announcement.registrationMode !== mode || announcement.personalDecks !== expected.personalDecks
    || announcement.registrationDeadline !== expected.registrationDeadline
    || announcement.checkInDeadline !== expected.checkInDeadline
    || !sameAnnouncementPlan(announcement.plan, expected.plan)
    || JSON.stringify(announcement.enabledRuleModules) !== JSON.stringify(expected.enabledRuleModules)
    || typeof announcement.initialHp !== 'number' || !Number.isSafeInteger(announcement.initialHp)
    || announcement.initialHp <= 0 || announcement.initialHp > 2147483647) {
    throw competitionCreateFromRulesError('engineProtocolError')
  }
  // 保留既有完整公開回應，但每一欄只取可信設定或已驗證的官方生命，不回傳 raw RPC。
  return {
    competitionId: request.competitionId, organizerUserId: request.callerUserId, revision: 1,
    projection: { ok: true, operation: 'registrationCreate', public: {
      announcement: {
        organizer: request.callerUserId, ruleVersion: expected.ruleVersion, ruleset: expected.ruleset,
        enabledRuleModules: [...expected.enabledRuleModules], teamSize: expected.teamSize,
        initialHp: announcement.initialHp, registrationMode: mode, personalDecks: expected.personalDecks,
        registrationDeadline: expected.registrationDeadline, checkInDeadline: expected.checkInDeadline,
        plan: structuredClone(expected.plan) as unknown as JsonObject,
      },
      lifecycle: 'draft', revision: 0, teams: [],
    } },
  }
}

function capability(value: unknown): value is object {
  // 平台 namespace／stub 可能是 class 或 function，不能套 JSON prototype 限制。
  return value !== null && !Array.isArray(value) && (typeof value === 'object' || typeof value === 'function')
}

async function dispatch(event: H3Event, request: CompetitionRoomInitializeFromRulesRequest): Promise<unknown> {
  try {
    const namespace: unknown = workerEnv(event).COMPETITION_ROOM
    if (!capability(namespace)) throw new Error('unavailable')
    const idFromName: unknown = Reflect.get(namespace, 'idFromName')
    const get: unknown = Reflect.get(namespace, 'get')
    if (typeof idFromName !== 'function' || typeof get !== 'function') throw new Error('unavailable')
    const id: unknown = Reflect.apply(idFromName, namespace, [request.competitionId])
    const room: unknown = Reflect.apply(get, namespace, [id])
    if (!capability(room)) throw new Error('unavailable')
    const initialize: unknown = Reflect.get(room, 'initializeFromRules')
    if (typeof initialize !== 'function') throw new Error('unavailable')
    // 捕捉方法並保留 receiver；唯一專用 RPC 呼叫，不 fallback 或自動重試。
    return await Reflect.apply(initialize, room, [structuredClone(request)])
  } catch {
    throw competitionCreateFromRulesError('competitionRoomUnavailable')
  }
}

function rpcResult(raw: unknown, request: CompetitionRoomInitializeFromRulesRequest): CompetitionRoomInitializeValue {
  let disposer: ((this: unknown) => unknown) | undefined
  let rejection: keyof typeof ERRORS = 'engineProtocolError'
  try {
    if (!record(raw)) throw competitionCreateFromRulesError('engineProtocolError')
    const resource = Object.getOwnPropertyDescriptor(raw, Symbol.dispose)
    if (resource) {
      if (!Object.hasOwn(resource, 'value') || typeof resource.value !== 'function'
        || Object.getPrototypeOf(resource.value)?.constructor?.name !== 'Function') {
        throw competitionCreateFromRulesError('engineProtocolError')
      }
      disposer = resource.value
    }
    const ok = Object.getOwnPropertyDescriptor(raw, 'ok')?.value
    if (ok === true && exact(raw, ['ok', 'value'], true)) return publicValue(raw.value, request)
    if (ok !== false || !exact(raw, ['ok', 'error'], true) || !isExactCompetitionJson(raw.error)
      || !record(raw.error) || !exact(raw.error, ['code', 'message'])
      || typeof raw.error.code !== 'string' || typeof raw.error.message !== 'string') {
      throw competitionCreateFromRulesError('engineProtocolError')
    }
    const code = raw.error.code
    if (code.startsWith('engine:')) rejection = 'engineRejected'
    if (code === 'invalidRequest' || code === 'forbidden' || code === 'competitionIdMismatch'
      || code === 'alreadyInitialized') rejection = code
    if (code === 'organizerMismatch') rejection = 'forbidden'
    throw competitionCreateFromRulesError(rejection)
  } catch {
    // 非 DTO 例外同樣採固定 protocol error；已驗 typed refusal 只保留安全白名單機器碼。
    throw competitionCreateFromRulesError(rejection)
  } finally {
    // 已實證的 native root disposer：原 receiver、恰一次，成功／拒絕／malformed 均釋放。
    if (disposer) {
      try {
        if (disposer.call(raw) !== undefined) throw new Error('disposer must be synchronous and void')
      } catch {
        throw competitionCreateFromRulesError('engineProtocolError')
      }
    }
  }
}

export async function createCompetitionFromRules(
  event: H3Event,
  body: unknown,
  callerUserId: string,
): Promise<CompetitionCreateAcknowledgement> {
  const announcement = announcementDraft(body)
  if (!nonEmpty(callerUserId)) throw competitionCreateFromRulesError('forbidden')
  let competitionId: string
  let trustedTimeMs: number
  try {
    competitionId = crypto.randomUUID()
    trustedTimeMs = Date.now()
    if (!nonEmpty(competitionId) || !timestamp(trustedTimeMs)) throw new Error('invalid server source')
  } catch {
    throw competitionCreateFromRulesError('serverSourceUnavailable')
  }
  const request = { competitionId, callerUserId, announcement, trustedTimeMs }
  const initialized = rpcResult(await dispatch(event, request), request)
  return { competitionId: initialized.competitionId }
}
