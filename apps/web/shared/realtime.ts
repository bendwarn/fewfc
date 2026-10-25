import type {
  GameRoomEntryResponse, GameRoomLobbyResponse, GameRoomRequest, GameRoomResponse,
  PlayableActionsResponse, RuleVersion,
} from './game-room'
import type { CompetitionRoomMatchClockReadValue } from './competition-room'

export interface RoomOperations {
  state: { input: undefined; output: GameRoomResponse }
  command: {
    input: Pick<Extract<GameRoomRequest, { type: 'submitCommand' }>, 'commandId' | 'gameInstanceId' | 'transactionId' | 'action'>
    output: GameRoomResponse | PlayableActionsResponse
  }
  ready: { input: { ready: boolean }; output: GameRoomResponse }
  start: { input: undefined; output: GameRoomResponse }
  reset: { input: undefined; output: GameRoomResponse }
  leave: { input: undefined; output: GameRoomResponse }
  dissolve: { input: undefined; output: GameRoomResponse }
  remove: { input: { userId: string }; output: GameRoomResponse }
  rules: { input: { enabledRuleModules?: string[]; ruleVersion?: RuleVersion }; output: GameRoomResponse }
  competitionReady: { input: { ready: boolean }; output: { ready: boolean } }
  clock: { input: undefined; output: CompetitionRoomMatchClockReadValue }
}

export interface LobbyOperations {
  list: { input: undefined; output: GameRoomLobbyResponse }
  create: {
    input: { name?: string; access?: 'public' | 'private'; capacity?: 2 | 4; ruleVersion?: RuleVersion; enabledRuleModules?: string[] }
    output: GameRoomEntryResponse
  }
  joinCode: { input: { code: string }; output: GameRoomEntryResponse }
  enter: { input: { gameId: string; invite?: string }; output: GameRoomResponse }
  saveReplay: { input: { sourceGameId: string }; output: unknown }
}

export type SocketRequest<Operations> = {
  [Name in keyof Operations]: Operations[Name] extends { input: infer Input }
    ? { type: 'request'; requestId: string; operation: Name }
      & (Input extends undefined ? { input?: null | undefined } : { input: Input })
    : never
}[keyof Operations]

export type SocketResult =
  | { type: 'result'; requestId: string; ok: true; data: unknown }
  | { type: 'result'; requestId: string; ok: false; error: { statusCode: number; code: string; message: string } }

const object = (value: unknown): value is Record<string, unknown> => (
  value !== null && typeof value === 'object' && !Array.isArray(value)
)
const identity = (value: unknown): value is string => typeof value === 'string' && value.trim().length > 0 && value.length <= 200
const version = (value: unknown) => value === undefined || value === '5.16' || value === '5.17'
const modules = (value: unknown) => value === undefined || (Array.isArray(value) && value.length <= 100 && value.every(identity))
const keys = (value: Record<string, unknown>, allowed: string[]) => Object.keys(value).every(key => allowed.includes(key))

/** 明確列出可由玩家送入的操作，不能把內部 GameRoomRequest 直接當公開協定。 */
export function parseSocketRequest(value: unknown, channel: 'room'): SocketRequest<RoomOperations> | undefined
export function parseSocketRequest(value: unknown, channel: 'lobby'): SocketRequest<LobbyOperations> | undefined
export function parseSocketRequest(value: unknown, channel: 'room' | 'lobby') {
  if (!object(value) || value.type !== 'request' || !identity(value.requestId)
    || !keys(value, ['type', 'requestId', 'operation', 'input'])) return
  const input = value.input
  let valid = false
  if (channel === 'room') {
    switch (value.operation) {
      case 'state': case 'start': case 'reset': case 'leave': case 'dissolve': case 'clock':
        valid = input === undefined || input === null
        break
      case 'ready': case 'competitionReady':
        valid = object(input) && keys(input, ['ready']) && typeof input.ready === 'boolean'
        break
      case 'remove':
        valid = object(input) && keys(input, ['userId']) && identity(input.userId)
        break
      case 'rules':
        valid = object(input) && keys(input, ['ruleVersion', 'enabledRuleModules']) && version(input.ruleVersion) && modules(input.enabledRuleModules)
        break
      case 'command':
        valid = object(input) && keys(input, ['commandId', 'gameInstanceId', 'transactionId', 'action'])
          && identity(input.commandId) && identity(input.gameInstanceId) && identity(input.transactionId)
          && object(input.action) && typeof input.action.type === 'string'
          && ['performFormation', 'changeProfession', 'activateProfessionAbility', 'useSpiritSkill',
            'retrievePreviousTurnDiscard', 'passAction', 'advanceAutomatic', 'answerChoice',
            'chooseInitialPouch', 'triggerSecretStrategy', 'playableActions'].includes(input.action.type)
        break
    }
  } else {
    switch (value.operation) {
      case 'list': valid = input === undefined || input === null; break
      case 'create':
        valid = object(input) && keys(input, ['name', 'access', 'capacity', 'ruleVersion', 'enabledRuleModules'])
          && (input.name === undefined || (typeof input.name === 'string' && input.name.length <= 200))
          && (input.access === undefined || input.access === 'public' || input.access === 'private')
          && (input.capacity === undefined || input.capacity === 2 || input.capacity === 4)
          && version(input.ruleVersion) && modules(input.enabledRuleModules)
        break
      case 'joinCode': valid = object(input) && keys(input, ['code']) && identity(input.code); break
      case 'saveReplay': valid = object(input) && keys(input, ['sourceGameId']) && identity(input.sourceGameId); break
      case 'enter': valid = object(input) && keys(input, ['gameId', 'invite']) && identity(input.gameId)
        && (input.invite === undefined || identity(input.invite)); break
    }
  }
  return valid ? value as SocketRequest<RoomOperations> | SocketRequest<LobbyOperations> : undefined
}

export function isSocketResult(value: unknown): value is SocketResult {
  return object(value) && value.type === 'result' && identity(value.requestId)
    && (value.ok === true || (value.ok === false && object(value.error)
      && typeof value.error.statusCode === 'number' && typeof value.error.code === 'string'
      && typeof value.error.message === 'string'))
}
