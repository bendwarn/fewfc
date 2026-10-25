import { expect, type Page } from '@playwright/test'
import type { GameRoomResponse, OnlineGameAction } from '../../shared/game-room'
import type { RoomOperations } from '../../shared/realtime'

interface CommandExchange {
  requestId: string
  action: OnlineGameAction
  input: RoomOperations['command']['input']
  ok: boolean
  data?: GameRoomResponse
  error?: unknown
}
const exchanges = new WeakMap<Page, CommandExchange[]>()
const sentCommands = new WeakMap<Page, OnlineGameAction[]>()
const roomStates = new WeakMap<Page, GameRoomResponse[]>()

export function gameCommandRequests(page: Page) { return sentCommands.get(page) ?? [] }
export function roomStateFrames(page: Page) { return roomStates.get(page) ?? [] }

export function observeRoomSocket(page: Page) {
  if (exchanges.has(page)) return
  const received: CommandExchange[] = []
  exchanges.set(page, received)
  const sent: OnlineGameAction[] = []
  sentCommands.set(page, sent)
  const states: GameRoomResponse[] = []
  roomStates.set(page, states)
  page.on('websocket', socket => {
    if (!/\/api\/games\/[^/]+\/socket$/.test(socket.url())) return
    const requests = new Map<string, RoomOperations['command']['input']>()
    socket.on('framesent', ({ payload }) => {
      const message = JSON.parse(String(payload))
      if (message.type === 'request' && message.operation === 'command') {
        requests.set(message.requestId, message.input)
        sent.push(message.input.action)
      }
    })
    socket.on('framereceived', ({ payload }) => {
      const message = JSON.parse(String(payload))
      if (message.type === 'roomState') states.push(message.data)
      const input = message.type === 'result' && requests.get(message.requestId)
      if (input) received.push({ requestId: message.requestId, action: input.action, input, ok: message.ok, data: message.data, error: message.error })
    })
  })
}

export async function waitForGameCommand(page: Page, predicate: (action: OnlineGameAction) => boolean = action => action.type !== 'playableActions') {
  const received = exchanges.get(page)
  if (!received) throw new Error('Call observeRoomSocket before navigation')
  const from = received.length
  let result: CommandExchange | undefined
  await expect.poll(() => {
    result = received.slice(from).find(exchange => predicate(exchange.action))
    return Boolean(result)
  }, { message: '等待 WebSocket 指令結果' }).toBe(true)
  return result!
}
