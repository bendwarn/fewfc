import type {
  CardInstanceId,
  LocalGameResponse,
  PlayableFormation,
  PlayerId,
  PublicGameEvent,
  PublicGameState,
  RecordedDecision,
  ViewerId,
} from '~/types/fewfc'
import type {
  GameRoomMetadata,
  GameRoomResponse,
  GameRoomSocketMessage,
  OnlineGameAction,
} from '../../shared/game-room'

type ViewerRef = Ref<ViewerId>

interface FewfcWasmExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(len: number): number
  fewfc_dealloc(ptr: number, len: number): void
  fewfc_handle_request(ptr: number, len: number): bigint
}

let browserWasmInstance: WebAssembly.Instance | undefined

function emptyState(): PublicGameState {
  return {
    status: 'InProgress',
    turnNumber: 1,
    phase: 'TurnStart',
    currentPlayer: null,
    players: [],
    turnOrder: [],
    hp: [],
    hands: [],
    discard: [],
    coveredPassives: [],
    pendingChoice: null,
    shields: [],
    statuses: [],
  }
}

async function browserRulesEngine(): Promise<FewfcWasmExports> {
  if (!browserWasmInstance) {
    const response = await fetch('/fewfc.wasm')

    if (!response.ok) {
      throw new Error('無法載入規則引擎')
    }

    const source = await WebAssembly.instantiate(await response.arrayBuffer(), {}) as
      | WebAssembly.Instance
      | WebAssembly.WebAssemblyInstantiatedSource
    browserWasmInstance = source instanceof WebAssembly.Instance ? source : source.instance
  }

  return (browserWasmInstance as WebAssembly.Instance).exports as FewfcWasmExports
}

async function callBrowserGame(body: Record<string, unknown>): Promise<LocalGameResponse> {
  const wasm = await browserRulesEngine()
  const input = new TextEncoder().encode(JSON.stringify(body))
  const inputPtr = wasm.fewfc_alloc(input.length)

  new Uint8Array(wasm.memory.buffer).set(input, inputPtr)

  const packed = wasm.fewfc_handle_request(inputPtr, input.length)
  wasm.fewfc_dealloc(inputPtr, input.length)

  const outputPtr = Number(packed >> BigInt(32))
  const outputLen = Number(packed & BigInt(0xffffffff))
  const outputBytes = new Uint8Array(wasm.memory.buffer, outputPtr, outputLen)
  const output = new TextDecoder().decode(outputBytes)
  wasm.fewfc_dealloc(outputPtr, outputLen)

  const parsed = JSON.parse(output) as LocalGameResponse | { error: string }

  if ('error' in parsed) {
    throw new Error(parsed.error)
  }

  return parsed
}

async function callLocalGame(body: Record<string, unknown>): Promise<LocalGameResponse> {
  if (import.meta.client) {
    try {
      return await callBrowserGame(body)
    } catch (error) {
      if (!import.meta.dev) {
        throw error
      }
    }
  }

  return await $fetch<LocalGameResponse>('/api/local-game', {
    method: 'POST',
    body,
  })
}

export function useLocalGame(viewer: ViewerRef) {
  const record = ref<RecordedDecision[]>([])
  const state = ref<PublicGameState>(emptyState())
  const metadata = ref<GameRoomMetadata | null>(null)
  const onlineGameId = ref<string | null>(null)
  const publicEvents = ref<PublicGameEvent[]>([])
  const selectedCards = ref<CardInstanceId[]>([])
  const playableFormations = ref<PlayableFormation[]>([])
  const errorMessage = ref<string | null>(null)
  const isLoading = ref(false)
  const interaction = ref({
    canPass: false,
    hasOptionalEffect: false,
  })
  const canCancelPendingCommand = ref(false)
  const connectionState = ref<'idle' | 'connecting' | 'connected' | 'reconnecting'>('idle')
  const roomDissolved = ref(false)
  const firstPlayer = ref<PlayerId>('alice')
  const deckSeed = ref(crypto.randomUUID())
  let roomSocket: WebSocket | null = null
  let roomSocketGameId: string | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let reconnectAttempt = 0

  watch(viewer, (nextViewer) => {
    selectedCards.value = []
    playableFormations.value = []

    if (onlineGameId.value) {
      void refreshOnlineGame()
      return
    }

    if (record.value.length > 0) {
      void refresh(nextViewer)
    }
  })

  watch(selectedCards, () => {
    if (selectedCards.value.length === 0 || viewer.value !== state.value.currentPlayer) {
      playableFormations.value = []
      return
    }

    void queryPlayableFormations()
  })

  function canSelectCard(player: PlayerId): boolean {
    return viewer.value === player && state.value.currentPlayer === player && !state.value.pendingChoice
  }

  function applyResponse(response: LocalGameResponse) {
    metadata.value = null
    onlineGameId.value = null
    record.value = response.record
    state.value = response.state
    publicEvents.value = response.events
    playableFormations.value = response.playableFormations
    interaction.value = response.interaction
    canCancelPendingCommand.value = false
    errorMessage.value = null
  }

  function applyRoomResponse(response: GameRoomResponse) {
    const changedRoom = onlineGameId.value !== response.gameId
    metadata.value = response.metadata
    onlineGameId.value = response.gameId
    state.value = response.state
    publicEvents.value = response.events
    playableFormations.value = response.playableFormations
    interaction.value = response.interaction
    canCancelPendingCommand.value = response.canCancelPendingCommand
    errorMessage.value = null
    roomDissolved.value = response.metadata.status === 'Dissolved'

    if (changedRoom && import.meta.client) {
      connectRoomSocket(response.gameId)
    }
  }

  async function submit(body: Record<string, unknown>) {
    isLoading.value = true
    errorMessage.value = null

    try {
      applyResponse(await callLocalGame(body))
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '規則引擎呼叫失敗'
    } finally {
      isLoading.value = false
    }
  }

  async function submitOnline(action: OnlineGameAction): Promise<boolean> {
    if (!onlineGameId.value) {
      return false
    }

    isLoading.value = true
    errorMessage.value = null

    try {
      applyRoomResponse(await $fetch<GameRoomResponse>(`/api/games/${onlineGameId.value}/commands`, {
        method: 'POST',
        body: {
          action,
        },
      }))
      return true
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '線上房間呼叫失敗'
      return false
    } finally {
      isLoading.value = false
    }
  }

  async function refresh(nextViewer = viewer.value) {
    if (onlineGameId.value) {
      await refreshOnlineGame()
      return
    }

    await submit({
      action: { type: 'refresh' },
      viewer: nextViewer,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
      record: record.value,
    })
  }

  async function refreshOnlineGame() {
    if (!onlineGameId.value) {
      return
    }

    isLoading.value = true
    errorMessage.value = null

    try {
      applyRoomResponse(await $fetch<GameRoomResponse>(`/api/games/${onlineGameId.value}`))
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '無法更新線上房間'
    } finally {
      isLoading.value = false
    }
  }

  async function startOnlineGame() {
    if (!onlineGameId.value) {
      return
    }

    isLoading.value = true
    errorMessage.value = null

    try {
      applyRoomResponse(await $fetch<GameRoomResponse>(`/api/games/${onlineGameId.value}/start`, {
        method: 'POST',
      }))
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '無法開始線上遊戲'
    } finally {
      isLoading.value = false
    }
  }

  async function toggleReady() {
    await roomMutation('ready')
  }

  async function leaveOnlineRoom() {
    const successful = await roomMutation('leave')

    if (successful) {
      disconnectRoomSocket()
      metadata.value = null
      onlineGameId.value = null
    }

    return successful
  }

  async function removeOnlinePlayer(userId: string) {
    return await roomMutation('remove', { userId })
  }

  async function dissolveOnlineRoom() {
    const successful = await roomMutation('dissolve')

    if (successful) {
      disconnectRoomSocket()
      roomDissolved.value = true
    }

    return successful
  }

  async function resetOnlineRoom() {
    return await roomMutation('reset')
  }

  async function cancelPendingCommand() {
    return await roomMutation('cancel-choice')
  }

  async function roomMutation(path: string, body?: Record<string, unknown>): Promise<boolean> {
    if (!onlineGameId.value) {
      return false
    }

    isLoading.value = true
    errorMessage.value = null

    try {
      const response = await $fetch<GameRoomResponse>(
        `/api/games/${onlineGameId.value}/${path}`,
        {
          method: 'POST',
          body,
        },
      )
      applyRoomResponse(response)
      return true
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '房間操作失敗'
      return false
    } finally {
      isLoading.value = false
    }
  }

  async function startSampleGame(nextFirstPlayer: PlayerId = 'alice') {
    metadata.value = null
    onlineGameId.value = null
    firstPlayer.value = nextFirstPlayer
    deckSeed.value = crypto.randomUUID()
    selectedCards.value = []
    await submit({
      action: { type: 'start' },
      viewer: viewer.value,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
    })
  }

  async function passAction() {
    if (onlineGameId.value) {
      if (await submitOnline({ type: 'passAction' })) {
        selectedCards.value = []
      }
      return
    }

    await submit({
      action: { type: 'passAction' },
      viewer: viewer.value,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
      record: record.value,
    })
  }

  async function advanceAutomatic() {
    selectedCards.value = []

    if (onlineGameId.value) {
      await submitOnline({ type: 'advanceAutomatic' })
      return
    }

    await submit({
      action: { type: 'advanceAutomatic' },
      viewer: viewer.value,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
      record: record.value,
    })
  }

  async function queryPlayableFormations() {
    const player = state.value.currentPlayer

    if (!player) {
      return
    }

    if (onlineGameId.value) {
      await submitOnline({
        type: 'playableFormations',
        player,
        cards: selectedCards.value,
      })
      return
    }

    await submit({
      action: {
        type: 'playableFormations',
        player,
        cards: selectedCards.value,
      },
      viewer: viewer.value,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
      record: record.value,
    })
  }

  async function choosePendingCard(card: CardInstanceId) {
    const choice = state.value.pendingChoice

    if (!choice || viewer.value !== choice.player) {
      return
    }

    if (choice.kind === 'TurnDrawDiscard') {
      if (onlineGameId.value) {
        if (await submitOnline({
          type: 'chooseTurnDiscard',
          player: choice.player,
          card,
        })) {
          selectedCards.value = []
        }
        return
      }

      await submit({
        action: {
          type: 'chooseTurnDiscard',
          player: choice.player,
          card,
        },
        viewer: viewer.value,
        firstPlayer: firstPlayer.value,
        deckSeed: deckSeed.value,
        record: record.value,
      })
      selectedCards.value = []
      return
    }

    if (choice.kind === 'EffectGenerated') {
      if (onlineGameId.value) {
        if (await submitOnline({
          type: 'answerEffectChoice',
          player: choice.player,
          cards: [card],
        }) && !canCancelPendingCommand.value) {
          selectedCards.value = []
        }
        return
      }

      await submit({
        action: {
          type: 'answerEffectChoice',
          player: choice.player,
          cards: [card],
        },
        viewer: viewer.value,
        firstPlayer: firstPlayer.value,
        deckSeed: deckSeed.value,
        record: record.value,
      })
      selectedCards.value = []
    }
  }

  function toggleCardSelection(player: PlayerId, card: CardInstanceId) {
    if (!canSelectCard(player)) {
      return
    }

    selectedCards.value = selectedCards.value.includes(card)
      ? selectedCards.value.filter((selected) => selected !== card)
      : [...selectedCards.value, card]
  }

  async function performFormation(formation: PlayableFormation) {
    const player = state.value.currentPlayer

    if (viewer.value !== player || selectedCards.value.length === 0) {
      return
    }

    const cards = [...selectedCards.value]

    if (onlineGameId.value) {
      if (await submitOnline({
        type: 'performFormation',
        player,
        formationId: formation.id,
        cards,
      }) && !canCancelPendingCommand.value) {
        selectedCards.value = []
      }
      return
    }

    await submit({
      action: {
        type: 'performFormation',
        player,
        formationId: formation.id,
        cards,
      },
      viewer: viewer.value,
      firstPlayer: firstPlayer.value,
      deckSeed: deckSeed.value,
      record: record.value,
    })
    selectedCards.value = []
  }

  function connectRoomSocket(gameId = onlineGameId.value) {
    if (!import.meta.client || !gameId) {
      return
    }

    if (
      roomSocket
      && roomSocket.readyState <= WebSocket.OPEN
      && roomSocketGameId === gameId
    ) {
      return
    }

    clearTimeout(reconnectTimer)
    connectionState.value = reconnectAttempt > 0 ? 'reconnecting' : 'connecting'
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = new WebSocket(
      `${protocol}//${window.location.host}/api/games/${encodeURIComponent(gameId)}/socket`,
    )
    roomSocket = socket
    roomSocketGameId = gameId

    socket.addEventListener('open', () => {
      reconnectAttempt = 0
      connectionState.value = 'connected'
    })

    socket.addEventListener('message', (event) => {
      if (typeof event.data !== 'string' || event.data === 'pong') {
        return
      }

      const message = JSON.parse(event.data) as GameRoomSocketMessage

      if (message.type === 'roomState') {
        applyRoomResponse(message.data)
        return
      }

      roomDissolved.value = true
      disconnectRoomSocket()
    })

    socket.addEventListener('close', () => {
      if (roomSocket !== socket || onlineGameId.value !== gameId || roomDissolved.value) {
        return
      }

      reconnectAttempt += 1
      connectionState.value = 'reconnecting'
      reconnectTimer = setTimeout(
        () => connectRoomSocket(gameId),
        Math.min(1000 * (2 ** (reconnectAttempt - 1)), 10000),
      )
    })

    socket.addEventListener('error', () => {
      socket.close()
    })
  }

  function disconnectRoomSocket() {
    clearTimeout(reconnectTimer)
    reconnectAttempt = 0
    const socket = roomSocket
    roomSocket = null
    roomSocketGameId = null
    connectionState.value = 'idle'

    if (socket && socket.readyState < WebSocket.CLOSING) {
      socket.close(1000, 'room closed')
    }
  }

  onBeforeUnmount(disconnectRoomSocket)

  return {
    metadata,
    onlineGameId,
    state,
    publicEvents,
    selectedCards,
    playableFormations,
    errorMessage,
    isLoading,
    interaction,
    canCancelPendingCommand,
    connectionState,
    roomDissolved,
    startSampleGame,
    applyRoomResponse,
    refreshOnlineGame,
    startOnlineGame,
    toggleReady,
    leaveOnlineRoom,
    removeOnlinePlayer,
    dissolveOnlineRoom,
    resetOnlineRoom,
    cancelPendingCommand,
    disconnectRoomSocket,
    passAction,
    advanceAutomatic,
    canSelectCard,
    toggleCardSelection,
    performFormation,
    choosePendingCard,
  }
}
