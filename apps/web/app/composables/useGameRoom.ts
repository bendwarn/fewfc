import type {
  CardInstanceId,
  PlayableFormation,
  PlayerId,
  PublicGameEvent,
  PublicGameState,
  ViewerId,
} from '~/types/fewfc'
import type {
  GameRoomMetadata,
  GameRoomInvitation,
  GameRoomResponse,
  GameRoomSocketMessage,
  OnlineGameAction,
} from '../../shared/game-room'

type ViewerRef = Ref<ViewerId>

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
    previousTurnFormation: null,
  }
}

export function useGameRoom(viewer: ViewerRef) {
  const state = ref<PublicGameState>(emptyState())
  const metadata = ref<GameRoomMetadata | null>(null)
  const invitation = ref<GameRoomInvitation | null>(null)
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
  let roomSocket: WebSocket | null = null
  let roomSocketGameId: string | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let reconnectAttempt = 0

  watch(viewer, () => {
    selectedCards.value = []
    playableFormations.value = []

    if (onlineGameId.value) {
      void refreshOnlineGame()
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

  function applyRoomResponse(response: GameRoomResponse) {
    metadata.value = response.metadata
    invitation.value = response.invitation ?? null
    onlineGameId.value = response.gameId
    state.value = response.state
    publicEvents.value = response.events
    playableFormations.value = response.playableFormations
    interaction.value = response.interaction
    canCancelPendingCommand.value = response.canCancelPendingCommand
    errorMessage.value = null
    roomDissolved.value = response.metadata.status === 'Dissolved'

    if (import.meta.client) {
      connectRoomSocket(response.gameId)
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
      clearRoom()
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

  async function passAction() {
    if (await submitOnline({ type: 'passAction' })) {
      selectedCards.value = []
    }
  }

  async function queryPlayableFormations() {
    const player = state.value.currentPlayer

    if (!player) {
      return
    }

    await submitOnline({
      type: 'playableFormations',
      player,
      cards: selectedCards.value,
    })
  }

  async function choosePendingCard(card: CardInstanceId) {
    const choice = state.value.pendingChoice

    if (!choice || viewer.value !== choice.player) {
      return
    }

    if (choice.kind === 'TurnDrawDiscard') {
      if (await submitOnline({
        type: 'chooseTurnDiscard',
        player: choice.player,
        card,
      })) {
        selectedCards.value = []
      }
      return
    }

    if (choice.kind === 'EffectGenerated') {
      if (await submitOnline({
        type: 'answerEffectChoice',
        player: choice.player,
        cards: [card],
      }) && !canCancelPendingCommand.value) {
        selectedCards.value = []
      }
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

    if (await submitOnline({
      type: 'performFormation',
      player,
      formationId: formation.id,
      cards,
    }) && !canCancelPendingCommand.value) {
      selectedCards.value = []
    }
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

  function clearRoom() {
    disconnectRoomSocket()
    metadata.value = null
    invitation.value = null
    onlineGameId.value = null
    state.value = emptyState()
    publicEvents.value = []
    selectedCards.value = []
    playableFormations.value = []
    errorMessage.value = null
    canCancelPendingCommand.value = false
    roomDissolved.value = false
  }

  onBeforeUnmount(disconnectRoomSocket)

  return {
    metadata,
    invitation,
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
    applyRoomResponse,
    refreshOnlineGame,
    startOnlineGame,
    toggleReady,
    leaveOnlineRoom,
    removeOnlinePlayer,
    dissolveOnlineRoom,
    resetOnlineRoom,
    cancelPendingCommand,
    clearRoom,
    disconnectRoomSocket,
    passAction,
    canSelectCard,
    toggleCardSelection,
    performFormation,
    choosePendingCard,
  }
}
