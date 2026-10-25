import { ref } from 'vue'
import type { GameRoomResponse, OnlineGameAction } from '../../shared/game-room'
import type { PlayableAction } from '../../app/types/fewfc'

type OperationInput<O, K extends keyof O> = O[K] extends { input: infer I } ? I : never
type OperationOutput<O, K extends keyof O> = O[K] extends { output: infer R } ? R : never

let response: GameRoomResponse
let actions: PlayableAction[] = []
let releaseQuery: (() => void) | undefined
export const roomCommands: OnlineGameAction[] = []
export const roomQueryMode = ref<'normal' | 'pending' | 'error'>('normal')

// 只替換傳輸邊界；房間頁與 useGameRoom 的選牌、草稿、提示及事件處理使用正式實作。
export function configureRoom(value: GameRoomResponse, offeredActions: PlayableAction[] = []) {
  releaseRoomQuery()
  response = value
  actions = offeredActions
  roomCommands.length = 0
  roomQueryMode.value = 'normal'
}

export function releaseRoomQuery() {
  releaseQuery?.()
  releaseQuery = undefined
}

export function useLayoutNotifications() {
  return {
    visibleNotifications: ref([]),
    dismissRoom() {},
    async request(operation: string) {
      if (operation === 'enter') return structuredClone(response)
      throw new Error(`房間故事未設定通知操作：${operation}`)
    },
  }
}

export function useRealtimeSocket<O>(_onMessage: (value: unknown, reconnected: boolean) => void) {
  return {
    connectionState: ref('connected'),
    connect() {},
    close() {},
    async request<K extends keyof O>(operation: K, input: OperationInput<O, K>): Promise<OperationOutput<O, K>> {
      if (operation === 'state') return structuredClone(response) as OperationOutput<O, K>
      if (operation !== 'command') throw new Error(`房間故事未設定操作：${String(operation)}`)
      const action = (input as { action: OnlineGameAction }).action
      roomCommands.push(structuredClone(action))
      if (action.type === 'playableActions') {
        if (roomQueryMode.value === 'pending') await new Promise<void>(resolve => { releaseQuery = resolve })
        if (roomQueryMode.value === 'error') throw new Error('query failed')
        return { playableActions: action.cards.length ? actions : response.playableActions } as OperationOutput<O, K>
      }
      throw new Error(`房間故事未設定命令回應：${action.type}`)
    },
  }
}
