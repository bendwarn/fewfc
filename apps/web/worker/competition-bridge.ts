import {
  isJsonObject,
  isJsonValue,
  isExactCompetitionJson,
} from '../shared/competition'
import type {
  CompetitionEngineError,
  ClockCheckpointMetadata,
  CompetitionEngineResponse,
  CompetitionEngineSuccess,
  CompetitionOperation,
  CompetitionOperationName,
  CompetitionProjectionAudience,
  CompetitionRequest,
  CompetitionServerState,
  JsonObject,
  PublicCompetitionResponse,
  PublicCompetitionSuccess,
  QualifiedStageTemplate,
  ResolvedStageQualification,
  ResolvedCompetitionSettings,
  StageQualificationSource,
} from '../shared/competition'
import { callCompetitionEngineWasm } from './rules-engine'

export type CompetitionWasmCall = (request: CompetitionRequest) => Promise<unknown>

type QualifiedStageOperation = Extract<CompetitionOperation, {
  type: 'tournamentPreviewQualifiedStage' | 'tournamentCreateQualifiedStage'
}>
type QualifiedStageOperationName = QualifiedStageOperation['type']

interface QualifiedStageExpectation {
  operation: QualifiedStageOperationName
  template: QualifiedStageTemplate
  serverState: CompetitionServerState
}

/**
 * 僅供伺服器使用的原始橋接。HTTP handler 不得直接回傳此值：
 * 其中包含 `serverState`，且可能含私人報名資料。
 */
export async function callCompetitionEngine(
  request: CompetitionRequest,
  wasmCall: CompetitionWasmCall = callCompetitionEngineWasm,
): Promise<CompetitionEngineResponse> {
  const metadata = request.operation.type === 'clockCheckpointMetadata'
  const settings = request.operation.type === 'registrationResolveSettings'
  const qualifiedStage = isQualifiedStageOperation(request.operation)
  if (qualifiedStage && !isQualifiedStageRequest(request)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage request is invalid')
  }
  if (settings && !isRulesSettingsRequest(request)) {
    throw new CompetitionBridgeProtocolError('competition engine rules settings request is invalid')
  }
  if (metadata && (!isJsonObject(request.operation)
    || !hasDisplayKeys(request.operation, ['type'])
    || !isMetadataServerState(request.serverState))) {
    throw new CompetitionBridgeProtocolError('competition engine metadata request is invalid')
  }
  const originalState = metadata || settings ? structuredClone(request.serverState) : null
  const qualifiedExpectation = qualifiedStage ? createQualifiedStageExpectation(request) : null
  const outbound = qualifiedStage
    ? structuredClone(request)
    : metadata || settings ? structuredClone(request) : request
  const raw = await wasmCall(outbound)
  if (qualifiedExpectation) return parseQualifiedStageCallResponse(raw, qualifiedExpectation)
  if (settings && (!isJsonObject(raw) || !isExactCompetitionJson(raw)
    || (raw.ok === false && (!hasDisplayKeys(raw, ['ok', 'error'])
      || !isJsonObject(raw.error) || !hasDisplayKeys(raw.error, ['code', 'layer', 'message']))))) {
    throw new CompetitionBridgeProtocolError('competition engine rules settings response is invalid')
  }
  if (metadata && (!isJsonObject(raw)
    || (raw.ok === false && (!hasDisplayKeys(raw, ['ok', 'error'])
      || !isJsonObject(raw.error) || !hasDisplayKeys(raw.error, ['code', 'layer', 'message']))))) {
    throw new CompetitionBridgeProtocolError('competition engine metadata response is invalid')
  }
  const response = parseCompetitionEngineResponse(raw)
  if (settings && response.ok && (response.operation !== 'registrationResolveSettings'
    || !sameMetadataJson(response.serverState, originalState))) {
    throw new CompetitionBridgeProtocolError('competition engine rules settings changed checkpoint')
  }
  if (metadata && response.ok && (response.operation !== 'clockCheckpointMetadata'
    || !sameMetadataJson(response.serverState, originalState))) {
    throw new CompetitionBridgeProtocolError('competition engine metadata changed checkpoint')
  }
  return response
}

function createQualifiedStageExpectation(request: CompetitionRequest): QualifiedStageExpectation {
  const snapshot = structuredClone(request)
  if (!isQualifiedStageOperation(snapshot.operation)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage request is invalid')
  }
  return {
    operation: snapshot.operation.type,
    template: structuredClone(snapshot.operation.template),
    serverState: structuredClone(snapshot.serverState),
  }
}

function parseQualifiedStageCallResponse(
  raw: unknown,
  expected: QualifiedStageExpectation,
): CompetitionEngineResponse {
  if (!isExactCompetitionJson(raw) || !isJsonObject(raw)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage response is invalid')
  }
  const ownedRaw = structuredClone(raw)
  if (!isJsonObject(ownedRaw)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage response is invalid')
  }
  if (ownedRaw.ok === false) {
    if (!isQualifiedStageFailureEnvelope(ownedRaw)) {
      throw new CompetitionBridgeProtocolError('competition engine qualified-stage error envelope is invalid')
    }
    return parseCompetitionEngineResponse(ownedRaw)
  }

  const response = parseCompetitionEngineResponse(ownedRaw)
  if (!response.ok || response.operation !== expected.operation || !isJsonObject(response.public)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage operation or qualification is invalid')
  }
  const publicProjection = response.public
  const qualification = publicProjection.qualification
  if (!isResolvedStageQualification(qualification)
    || !isQualificationForTemplate(qualification, expected.template)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage operation or qualification is invalid')
  }

  if (expected.operation === 'tournamentPreviewQualifiedStage') {
    const plannedStage = publicProjection.plannedStage
    if (!sameQualifiedStageJson(response.serverState, expected.serverState)
      || !isJsonObject(plannedStage) || !isJsonObject(plannedStage.config)
      || plannedStage.config.stageId !== expected.template.stageId
      || !Array.isArray(plannedStage.config.teams)
      // 橋接只比對 Rust 階段設定中的原序名單，不排序或重新推導資格。
      || !sameQualifiedStageJson(plannedStage.config.teams, qualification.seededTeams)) {
      throw new CompetitionBridgeProtocolError('competition engine qualified-stage preview changed checkpoint, stage, or teams')
    }
  } else {
    const currentTournament = publicProjection.currentTournament
    if (!sameQualifiedStageNonTournamentState(response.serverState, expected.serverState)
      || !isQualifiedStageTournamentSnapshot(currentTournament)
      || !isQualifiedStageTournamentSnapshot(response.serverState.tournament)
      || !sameQualifiedStageJson(currentTournament, response.serverState.tournament)) {
      throw new CompetitionBridgeProtocolError('competition engine qualified-stage create changed unrelated state')
    }
    // 不解析其他階段內容；只確認請求階段出現在 Rust 回傳的賽程中。
    if (!hasQualifiedStage(currentTournament, expected.template.stageId)) {
      throw new CompetitionBridgeProtocolError('competition engine qualified-stage create omitted the requested stage')
    }
  }
  return response
}

function isQualifiedStageFailureEnvelope(value: JsonObject): boolean {
  return isExactCompetitionJson(value) && hasDisplayKeys(value, ['ok', 'error'])
    && value.ok === false && isJsonObject(value.error)
    && hasDisplayKeys(value.error, ['code', 'layer', 'message'])
    && typeof value.error.code === 'string'
    && typeof value.error.layer === 'string'
    && typeof value.error.message === 'string'
}

function sameQualifiedStageNonTournamentState(
  actual: CompetitionServerState,
  expected: CompetitionServerState,
): boolean {
  return sameQualifiedStageJson(actual.registration, expected.registration)
    && sameQualifiedStageJson(actual.clock, expected.clock)
    && Object.hasOwn(actual, 'matches') === Object.hasOwn(expected, 'matches')
    && sameQualifiedStageJson(actual.matches, expected.matches)
}

function sameQualifiedStageJson(left: unknown, right: unknown): boolean {
  if (left === right) return true
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) && left.length === right.length
      && left.every((value, index) => sameQualifiedStageJson(value, right[index]))
  }
  if (!isJsonObject(left) || !isJsonObject(right)) return false
  const leftKeys = Reflect.ownKeys(left)
  const rightKeys = Reflect.ownKeys(right)
  return leftKeys.length === rightKeys.length && leftKeys.every(key => typeof key === 'string'
    && Object.hasOwn(right, key) && sameQualifiedStageJson(left[key], right[key]))
}

function isQualificationForTemplate(value: unknown, template: QualifiedStageTemplate): boolean {
  if (!isResolvedStageQualification(value) || value.seededTeams.length > template.sourceSlots.length
    || value.sourceMappings.length !== template.sourceSlots.length) return false
  const mappedTeams: Array<{ teamId: string, seed: number }> = []
  const validMappings = template.sourceSlots.every((slot, index) => {
    const mapping = value.sourceMappings[index]
    if (!mapping || mapping.sourceStageId !== slot.sourceStageId
      || mapping.destinationSeed !== slot.destinationSeed
      || !sameQualifiedStageJson(mapping.source, slot.source)) return false
    if (mapping.teamId === null) return true
    const seededTeam = value.seededTeams.find(team => team.teamId === mapping.teamId)
    if (!seededTeam || seededTeam.seed !== mapping.destinationSeed) return false
    mappedTeams.push({ teamId: mapping.teamId, seed: mapping.destinationSeed })
    return true
  })
  return validMappings && sameQualifiedStageJson(mappedTeams, value.seededTeams)
}

/**
 * 安全投影邊界。永遠回傳 `public`；`private` 僅供明確授權的受眾。
 * 不將 `serverState` 重建至任一分支。
 */
export function projectCompetitionResponse(
  response: CompetitionEngineResponse,
  audience: CompetitionProjectionAudience = { type: 'public' },
): PublicCompetitionResponse {
  if (!response.ok) return response

  const projected: PublicCompetitionSuccess = {
    ok: true,
    operation: response.operation,
    public: response.public,
  }
  if (audience.type === 'authorizedPrivate' && response.private !== undefined) {
    projected.private = response.private
  }
  return projected
}

/**
 * 提供給已決定受眾的內部呼叫端使用。
 * 仍只回傳投影，不回傳原始伺服器 checkpoint。
 */
export async function executeCompetitionOperation(
  request: CompetitionRequest,
  audience: CompetitionProjectionAudience = { type: 'public' },
  wasmCall: CompetitionWasmCall = callCompetitionEngineWasm,
): Promise<PublicCompetitionResponse> {
  return projectCompetitionResponse(await callCompetitionEngine(request, wasmCall), audience)
}

export function parseCompetitionEngineResponse(value: unknown): CompetitionEngineResponse {
  if (!isJsonObject(value)) {
    throw new CompetitionBridgeProtocolError('competition engine returned a non-object response')
  }

  if (value.ok === false) {
    return { ok: false, error: parseEngineError(value.error) }
  }
  if (value.ok !== true) {
    throw new CompetitionBridgeProtocolError('competition engine response is missing ok')
  }

  const operation = parseOperationName(value.operation)
  if (isQualifiedStageOperationName(operation) && !isQualifiedStageSuccessEnvelope(value, operation)) {
    throw new CompetitionBridgeProtocolError('competition engine qualified-stage success envelope is invalid')
  }
  if (operation === 'registrationResolveSettings'
    && (!isExactCompetitionJson(value)
      || !hasDisplayKeys(value, ['ok', 'operation', 'public', 'serverState'])
      || !isResolvedCompetitionSettings(value.public)
      || !isMetadataServerState(value.serverState))) {
    throw new CompetitionBridgeProtocolError('competition engine rules settings response is invalid')
  }
  if (!isJsonValue(value.public)) {
    throw new CompetitionBridgeProtocolError('competition engine response is missing public')
  }
  // 顯示 query 不得沿用可含裁判结果的 mutation 投影，也不可把額外私密欄位向外轉送。
  if (operation === 'clockView' && !isClockDisplay(value.public)) {
    throw new CompetitionBridgeProtocolError('competition engine clockView display is invalid')
  }
  if (operation === 'clockCheckpointMetadata'
    && (!hasDisplayKeys(value, ['ok', 'operation', 'public', 'serverState'])
      || !isClockCheckpointMetadata(value.public) || !isMetadataServerState(value.serverState))) {
    throw new CompetitionBridgeProtocolError('competition engine metadata response is invalid')
  }
  const serverState = parseServerState(value.serverState,
    operation === 'clockView' || operation === 'clockCheckpointMetadata'
      || operation === 'registrationResolveSettings' || isQualifiedStageOperationName(operation))
  const response: CompetitionEngineSuccess = {
    ok: true,
    operation,
    public: value.public,
    serverState,
  }
  if (value.private !== undefined) {
    if (!isJsonValue(value.private)) {
      throw new CompetitionBridgeProtocolError('competition engine private output is invalid')
    }
    response.private = value.private
  }
  return response
}

export class CompetitionBridgeProtocolError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'CompetitionBridgeProtocolError'
  }
}

function hasDisplayKeys(value: JsonObject, keys: string[]): boolean {
  const actual = Reflect.ownKeys(value)
  return actual.length === keys.length && keys.every(key => Object.hasOwn(value, key))
}

function isDisplayTime(value: unknown): boolean {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isDisplayLimit(value: unknown): boolean {
  return value === null || isDisplayTime(value)
}

export function isClockCheckpointMetadata(value: unknown): value is ClockCheckpointMetadata {
  return isJsonObject(value) && hasDisplayKeys(value, ['nextTimeoutDeadlineMs'])
    && isDisplayLimit(value.nextTimeoutDeadlineMs)
}

export function isResolvedCompetitionSettings(value: unknown): value is ResolvedCompetitionSettings {
  return isJsonObject(value) && isExactCompetitionJson(value)
    && hasDisplayKeys(value, ['initialHp'])
    && typeof value.initialHp === 'number' && Number.isSafeInteger(value.initialHp)
    && value.initialHp > 0 && value.initialHp <= 2147483647
}

function isRulesSettingsRequest(value: unknown): boolean {
  if (!isExactCompetitionJson(value) || !isJsonObject(value)
    || !hasDisplayKeys(value, ['serverActor', 'trustedTimeMs', 'operation', 'serverState'])
    || !isJsonObject(value.serverActor) || !hasDisplayKeys(value.serverActor, ['type', 'playerId'])
    || typeof value.serverActor.type !== 'string'
    || !['organizer', 'referee', 'player'].includes(value.serverActor.type)
    || typeof value.serverActor.playerId !== 'string' || !value.serverActor.playerId.trim()
    || !isDisplayTime(value.trustedTimeMs) || !isMetadataServerState(value.serverState)) return false
  const operation = value.operation
  if (!isJsonObject(operation) || operation.type !== 'registrationResolveSettings') return false
  return hasDisplayKeys(operation, [
    'type', 'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize',
  ]) && (operation.ruleVersion === '5.16' || operation.ruleVersion === '5.17')
    && typeof operation.ruleset === 'string' && operation.ruleset.trim().length > 0
    && Array.isArray(operation.enabledRuleModules)
    && operation.enabledRuleModules.every(module => typeof module === 'string' && module.trim().length > 0)
    && new Set(operation.enabledRuleModules).size === operation.enabledRuleModules.length
    && typeof operation.teamSize === 'number' && Number.isSafeInteger(operation.teamSize)
    && operation.teamSize > 0 && operation.teamSize <= 65535
}

function isQualifiedStageOperation(operation: CompetitionOperation): operation is QualifiedStageOperation {
  return operation.type === 'tournamentPreviewQualifiedStage'
    || operation.type === 'tournamentCreateQualifiedStage'
}

function isQualifiedStageOperationName(value: unknown): value is QualifiedStageOperationName {
  return value === 'tournamentPreviewQualifiedStage' || value === 'tournamentCreateQualifiedStage'
}

function isQualifiedStageRequest(value: unknown): value is CompetitionRequest & { operation: QualifiedStageOperation } {
  if (!isExactCompetitionJson(value) || !isJsonObject(value)
    || !hasDisplayKeys(value, ['serverActor', 'trustedTimeMs', 'operation', 'serverState'])
    || typeof value.trustedTimeMs !== 'number'
    || !isJsonObject(value.serverActor)
    || !hasDisplayKeys(value.serverActor, ['type', 'playerId'])
    || typeof value.serverActor.type !== 'string'
    || !['organizer', 'player', 'referee'].includes(value.serverActor.type)
    || typeof value.serverActor.playerId !== 'string'
    || !isQualifiedStageServerState(value.serverState)
    || !isJsonObject(value.operation)
    || !hasDisplayKeys(value.operation, ['type', 'template'])
    || !isQualifiedStageOperationName(value.operation.type)
    || !isQualifiedStageTemplate(value.operation.template)) return false
  return true
}

function isQualifiedStageTemplate(value: unknown): value is QualifiedStageTemplate {
  if (!isJsonObject(value) || !hasDisplayKeys(value, [
    'stageId', 'format', 'randomSeed', 'matchFormat', 'clockConfig',
    'swissRounds', 'swissPolicy', 'sourceSlots',
  ]) || typeof value.stageId !== 'string'
    || typeof value.format !== 'string'
    || !['singleElimination', 'doubleElimination', 'roundRobin', 'swiss'].includes(value.format)
    || typeof value.randomSeed !== 'number'
    || !isQualifiedStageMatchFormat(value.matchFormat)
    || !isQualifiedStageClockConfig(value.clockConfig)
    || !(value.swissRounds === null || typeof value.swissRounds === 'number')
    || !(value.swissPolicy === null || isQualifiedStageSwissPolicy(value.swissPolicy))
    || !Array.isArray(value.sourceSlots)) return false
  return value.sourceSlots.every(isQualifiedStageSlot)
}

function isQualifiedStageMatchFormat(value: unknown): boolean {
  if (!isJsonObject(value) || !hasDisplayKeys(value, ['type', 'value']) || typeof value.value !== 'number') {
    return false
  }
  return value.type === 'fixedGames' || value.type === 'firstToWins'
}

function isQualifiedStageClockConfig(value: unknown): boolean {
  return isJsonObject(value)
    && hasDisplayKeys(value, ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'])
    && [value.mainTimeMs, value.actionTimeMs, value.outOfTurnChoiceTimeMs]
      .every(limit => limit === null || typeof limit === 'number')
}

function isQualifiedStageSwissPolicy(value: unknown): boolean {
  if (!isJsonObject(value) || !hasDisplayKeys(value, [
    'matchPoints', 'drawCredit', 'opponentWinRateFloorPercent', 'opponentWinRateCapPercent',
    'excludeByesFromOpponentWinRate', 'tieBreakers',
  ]) || !isJsonObject(value.matchPoints)
    || !hasDisplayKeys(value.matchPoints, ['win', 'draw', 'loss', 'bye'])
    || ![value.matchPoints.win, value.matchPoints.draw, value.matchPoints.loss, value.matchPoints.bye]
      .every(points => typeof points === 'number')
    || !isJsonObject(value.drawCredit)
    || !hasDisplayKeys(value.drawCredit, ['numerator', 'denominator'])
    || typeof value.drawCredit.numerator !== 'number'
    || typeof value.drawCredit.denominator !== 'number'
    || !(value.opponentWinRateFloorPercent === null || typeof value.opponentWinRateFloorPercent === 'number')
    || !(value.opponentWinRateCapPercent === null || typeof value.opponentWinRateCapPercent === 'number')
    || typeof value.excludeByesFromOpponentWinRate !== 'boolean'
    || !Array.isArray(value.tieBreakers)) return false
  const tieBreakers = ['matchPoints', 'headToHeadMatchPoints', 'opponentWinRate', 'netGameWins', 'seedOrder']
  return value.tieBreakers.every(value => typeof value === 'string' && tieBreakers.includes(value))
}

function isQualifiedStageSlot(value: unknown): value is QualifiedStageTemplate['sourceSlots'][number] {
  return isJsonObject(value)
    && hasDisplayKeys(value, ['sourceStageId', 'source', 'destinationSeed'])
    && typeof value.sourceStageId === 'string'
    && typeof value.destinationSeed === 'number'
    && isStageQualificationSource(value.source)
}

function isStageQualificationSource(value: unknown): value is StageQualificationSource {
  if (!isJsonObject(value) || typeof value.type !== 'string') return false
  if (value.type === 'finishedStageWinner') return hasDisplayKeys(value, ['type'])
  return value.type === 'uniqueStandingRank'
    && hasDisplayKeys(value, ['type', 'rank']) && typeof value.rank === 'number'
}

function isQualifiedStageServerState(value: unknown): value is CompetitionServerState {
  if (!isJsonObject(value)) return false
  const keys = Object.hasOwn(value, 'matches')
    ? ['registration', 'clock', 'tournament', 'matches'] : ['registration', 'clock', 'tournament']
  return hasDisplayKeys(value, keys)
    && (value.registration === null || isJsonObject(value.registration))
    && (value.clock === null || isJsonObject(value.clock))
    && (value.tournament === null || isJsonObject(value.tournament))
    && (!Object.hasOwn(value, 'matches') || isJsonObject(value.matches))
}

function isQualifiedStageTournamentSnapshot(value: unknown): value is JsonObject {
  return isJsonObject(value) && typeof value.tournamentId === 'string'
    && Array.isArray(value.stages)
}

function hasQualifiedStage(snapshot: JsonObject, stageId: string): boolean {
  return Array.isArray(snapshot.stages) && snapshot.stages.some(stage => isJsonObject(stage)
    && isJsonObject(stage.config) && stage.config.stageId === stageId)
}

function isQualifiedStageSuccessEnvelope(
  value: unknown,
  operation: QualifiedStageOperationName,
): value is JsonObject {
  return isExactCompetitionJson(value) && isJsonObject(value)
    && hasDisplayKeys(value, ['ok', 'operation', 'public', 'serverState'])
    && value.ok === true && value.operation === operation
    && isQualifiedStagePublicProjection(value.public, operation)
    && isQualifiedStageServerState(value.serverState)
}

function isQualifiedStagePublicProjection(value: unknown, operation: QualifiedStageOperationName): boolean {
  if (!isJsonObject(value)
    || !hasDisplayKeys(value, operation === 'tournamentPreviewQualifiedStage'
      ? ['qualification', 'plannedStage'] : ['qualification', 'currentTournament'])
    || !isResolvedStageQualification(value.qualification)) return false
  if (operation === 'tournamentPreviewQualifiedStage') {
    return isJsonObject(value.plannedStage) && isJsonObject(value.plannedStage.config)
      && typeof value.plannedStage.config.stageId === 'string'
  }
  return isJsonObject(value.currentTournament)
    && isQualifiedStageTournamentSnapshot(value.currentTournament)
}

function isResolvedStageQualification(value: unknown): value is ResolvedStageQualification {
  return isJsonObject(value)
    && hasDisplayKeys(value, ['seededTeams', 'sourceMappings'])
    && Array.isArray(value.seededTeams) && value.seededTeams.every(isSeededTeam)
    && Array.isArray(value.sourceMappings) && value.sourceMappings.every(isStageQualificationMapping)
}

function isSeededTeam(value: unknown): boolean {
  return isJsonObject(value) && hasDisplayKeys(value, ['teamId', 'seed'])
    && typeof value.teamId === 'string' && typeof value.seed === 'number'
}

function isStageQualificationMapping(value: unknown): boolean {
  return isJsonObject(value)
    && hasDisplayKeys(value, ['sourceStageId', 'source', 'teamId', 'destinationSeed'])
    && typeof value.sourceStageId === 'string'
    && isStageQualificationSource(value.source)
    && (value.teamId === null || typeof value.teamId === 'string')
    && typeof value.destinationSeed === 'number'
}

function isMetadataJson(value: unknown): boolean {
  if (typeof value === 'number') return Number.isFinite(value)
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (Array.isArray(value)) {
    return Reflect.ownKeys(value).length === value.length + 1
      && Object.keys(value).length === value.length && value.every(isMetadataJson)
  }
  return isJsonObject(value) && Reflect.ownKeys(value).length === Object.keys(value).length
    && Object.values(value).every(isMetadataJson)
}

function isMetadataServerState(value: unknown): boolean {
  return isJsonObject(value) && isMetadataJson(value)
    && hasDisplayKeys(value, Object.hasOwn(value, 'matches')
      ? ['registration', 'clock', 'tournament', 'matches'] : ['registration', 'clock', 'tournament'])
}

function sameMetadataJson(left: unknown, right: unknown): boolean {
  if (left === right) return true
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.length === right.length && left.every((value, index) => sameMetadataJson(value, right[index]))
  }
  if (!isJsonObject(left) || !isJsonObject(right)) return false
  const keys = Object.keys(left)
  return keys.length === Object.keys(right).length
    && keys.every(key => Object.hasOwn(right, key) && sameMetadataJson(left[key], right[key]))
}

function isDisplayWindow(value: unknown, teams: string[]): boolean {
  if (value === null) return true
  if (!isJsonObject(value)) return false
  if (value.type === 'preparation') {
    return hasDisplayKeys(value, ['type', 'teams'])
      && Array.isArray(value.teams) && value.teams.length >= 1 && value.teams.length <= 2
      && new Set(value.teams).size === value.teams.length
      && value.teams.every(team => typeof team === 'string' && teams.includes(team))
  }
  if (typeof value.team !== 'string' || !teams.includes(value.team)) return false
  if (value.type === 'action') {
    return hasDisplayKeys(value, ['type', 'team', 'turn']) && isDisplayTime(value.turn)
  }
  if (value.type === 'outOfTurnChoice') {
    return hasDisplayKeys(value, ['type', 'team', 'choiceId', 'remainingMs'])
      && typeof value.choiceId === 'string' && value.choiceId.length > 0
      && isDisplayLimit(value.remainingMs)
  }
  return false
}

function isClockDisplay(value: unknown): boolean {
  if (!isJsonObject(value)
    || !hasDisplayKeys(value, ['sampledAtMs', 'teams', 'running', 'paused', 'awaitingAuthority'])
    || !isDisplayTime(value.sampledAtMs)
    || typeof value.awaitingAuthority !== 'boolean'
    || !Array.isArray(value.teams) || value.teams.length !== 2) return false
  const teams: string[] = []
  for (const entry of value.teams) {
    if (!isJsonObject(entry)
      || !hasDisplayKeys(entry, ['team', 'mainTimeRemainingMs', 'byoyomiTimeRemainingMs', 'byoyomiActive'])
      || typeof entry.team !== 'string' || entry.team.length === 0
      || !isDisplayLimit(entry.mainTimeRemainingMs) || !isDisplayLimit(entry.byoyomiTimeRemainingMs)
      || typeof entry.byoyomiActive !== 'boolean'
      || (entry.byoyomiActive && entry.mainTimeRemainingMs !== 0)) return false
    teams.push(entry.team)
  }
  return teams[0] !== teams[1]
    && (value.running === null || value.paused === null)
    && isDisplayWindow(value.running, teams) && isDisplayWindow(value.paused, teams)
}

function parseEngineError(value: unknown): CompetitionEngineError {
  if (!isJsonObject(value)
    || typeof value.code !== 'string'
    || typeof value.layer !== 'string'
    || typeof value.message !== 'string') {
    throw new CompetitionBridgeProtocolError('competition engine error envelope is invalid')
  }
  return {
    code: value.code,
    layer: value.layer,
    message: value.message,
  }
}

function parseOperationName(value: unknown): CompetitionOperationName {
  const names: CompetitionOperationName[] = [
    'registrationResolveSettings',
    'registrationCreate',
    'registrationExecute',
    'registrationView',
    'registrationExportLockedRoster',
    'clockCreate',
    'clockBeginPreparation',
    'clockFinishPreparation',
    'clockBeginAction',
    'clockBeginOutOfTurnChoice',
    'clockObserve',
    'clockView',
    'clockCheckpointMetadata',
    'clockEndWindow',
    'clockPause',
    'clockResume',
    'clockNewGame',
    'clockRuleGame',
    'tournamentCreate',
    'tournamentPreviewQualifiedStage',
    'tournamentCreateQualifiedStage',
    'tournamentView',
    'tournamentAdjudicateRankingTie',
    'tournamentStartMatch',
    'tournamentSubmitResult',
    'tournamentCorrectResult',
    'tournamentDropTeam',
    'matchCreate',
    'matchView',
    'matchAssignFirstPlayer',
    'matchSubmitGame',
    'matchCorrectRuling',
  ]
  if (isOperationName(value, names)) {
    return value
  }
  throw new CompetitionBridgeProtocolError('competition engine operation is invalid')
}

function isOperationName(
  value: unknown,
  names: CompetitionOperationName[],
): value is CompetitionOperationName {
  return typeof value === 'string' && names.some(name => name === value)
}

function parseServerState(value: unknown, preserveQueryShape = false): CompetitionServerState {
  if (!isJsonObject(value)) {
    throw new CompetitionBridgeProtocolError('competition engine serverState is invalid')
  }
  // 既有 mutation 保留舊版 matches 正規化；新 pure query 不得替保存狀態補預設值。
  const matches = preserveQueryShape && value.matches === undefined
    ? undefined
    : optionalObject(value.matches, 'matches')
  return {
    registration: nullableObject(value.registration, 'registration'),
    clock: nullableObject(value.clock, 'clock'),
    tournament: nullableObject(value.tournament, 'tournament'),
    ...(matches === undefined ? {} : { matches }),
  }
}

function nullableObject(value: unknown, name: string): JsonObject | null {
  if (value === null) return null
  if (isJsonObject(value)) return value
  throw new CompetitionBridgeProtocolError(`competition engine serverState.${name} is invalid`)
}

function optionalObject(value: unknown, name: string): JsonObject {
  if (value === undefined) return {}
  if (isJsonObject(value)) return value
  throw new CompetitionBridgeProtocolError('competition engine serverState.' + name + ' is invalid')
}
