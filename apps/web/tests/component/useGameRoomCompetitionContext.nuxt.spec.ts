import { mountSuspended } from '@nuxt/test-utils/runtime'
import { defineComponent, h, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { emptyPublicState, type GameRoomResponse } from '#shared/game-room'
import type { ViewerId } from '~/types/fewfc'
import { useGameRoom } from '~/composables/useGameRoom'

class FakeWebSocket extends EventTarget {
  static OPEN = 1
  static CLOSING = 2
  readyState = FakeWebSocket.OPEN

  constructor(readonly url: string) { super() }

  close() {
    this.readyState = 3
  }

  receive(response: GameRoomResponse) {
    this.dispatchEvent(new MessageEvent('message', {
      data: JSON.stringify({ type: 'roomState', data: response }),
    }))
  }
}

function waitingResponse(competition?: GameRoomResponse['competition']): GameRoomResponse {
  const response: GameRoomResponse = {
    gameId: 'room-a',
    metadata: {
      schemaVersion: 5,
      gameId: 'room-a',
      name: '固定房間名稱',
      access: 'public',
      capacity: 2,
      ruleVersion: '5.16',
      ruleset: 'fewfc-base',
      enabledRuleModules: [],
      players: ['alice', 'bob'],
      members: [
        { userId: 'member-a', displayName: 'A', player: 'alice', ready: true, connected: true, owner: true },
        { userId: 'member-b', displayName: 'B', player: 'bob', ready: false, connected: true, owner: false },
      ],
      observers: [],
      status: 'Waiting',
      createdAt: '2026-10-01T00:00:00.000Z',
      updatedAt: '2026-10-01T00:00:00.000Z',
    },
    state: emptyPublicState(['alice', 'bob']),
    battleRecord: { preparation: { entries: [] }, turns: [] },
    playableActions: [],
    interaction: { canChooseInitialPouch: false },
  }
  return competition ? { ...response, competition } : response
}

function installWebSocket(sockets: FakeWebSocket[]) {
  vi.stubGlobal('WebSocket', class extends FakeWebSocket {
    constructor(url: string) {
      super(url)
      sockets.push(this)
    }
  })
}

async function mountRoom() {
  const capture: { room?: ReturnType<typeof useGameRoom> } = {}
  const component = defineComponent({
    setup() {
      const viewer = ref<ViewerId>('observer')
      capture.room = useGameRoom(viewer)
      return () => h('div')
    },
  })
  const wrapper = await mountSuspended(component)
  const room = capture.room
  if (!room) throw new Error('room composable was not mounted')
  return {
    wrapper,
    room,
  }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('updates competition mode from accepted HTTP and WebSocket room responses, then clears it with the room', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, room } = await mountRoom()

  room.applyRoomResponse(waitingResponse({
    competitionId: 'competition-a',
    matchId: 'match-a',
    gameNumber: 1,
  }))
  expect(room.competitionMode.value).toEqual({
    kind: 'competition',
    context: { competitionId: 'competition-a', matchId: 'match-a', gameNumber: 1 },
  })

  const socket = sockets[0]
  if (!socket) throw new Error('room response did not connect a WebSocket')
  socket.receive(waitingResponse({
    competitionId: 'competition-b',
    matchId: 'match-b',
    gameNumber: 2,
  }))
  expect(room.competitionMode.value).toEqual({
    kind: 'competition',
    context: { competitionId: 'competition-b', matchId: 'match-b', gameNumber: 2 },
  })

  socket.receive(waitingResponse())
  expect(room.competitionMode.value).toEqual({ kind: 'ordinary' })

  room.applyRoomResponse(waitingResponse({
    competitionId: 'competition-a',
    matchId: 'match-a',
    gameNumber: 1,
  }))
  room.clearRoom()
  expect(room.competitionMode.value).toEqual({ kind: 'ordinary' })
  expect(room.onlineGameId.value).toBeNull()
  await wrapper.unmount()
})

it('refreshes the trusted competition context from a reconnected room socket', async () => {
  vi.useFakeTimers()
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, room } = await mountRoom()
  room.applyRoomResponse(waitingResponse({
    competitionId: 'competition-a',
    matchId: 'match-a',
    gameNumber: 1,
  }))

  const firstSocket = sockets[0]
  if (!firstSocket) throw new Error('room response did not connect a WebSocket')
  firstSocket.readyState = 3
  firstSocket.dispatchEvent(new Event('close'))
  await vi.advanceTimersByTimeAsync(1000)

  const reconnectedSocket = sockets[1]
  if (!reconnectedSocket) throw new Error('room reconnect did not create a new WebSocket')
  reconnectedSocket.receive(waitingResponse({
    competitionId: 'competition-a',
    matchId: 'match-a',
    gameNumber: 2,
  }))
  expect(room.competitionMode.value).toEqual({
    kind: 'competition',
    context: { competitionId: 'competition-a', matchId: 'match-a', gameNumber: 2 },
  })
  await wrapper.unmount()
})
