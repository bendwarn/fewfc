import type {
  CardInstanceId,
  Element,
  PlayableAction,
  PlayerId,
  PublicGameEvent,
  PublicGameState,
  SecretStrategy,
  StarKind,
  ViewerId,
} from '~/types/fewfc'
import type {
  GameRoomMetadata,
  GameRoomInvitation,
  GameRoomResponse,
  GameRoomSocketMessage,
  OnlineGameAction,
} from '../../shared/game-room'
import {
  togglePendingChoiceSelection,
} from '~/lib/pending-choice-selection'
import { reconcileActionDraft, toggleActionDraftCard } from '~/lib/action-draft'
import { presentApiError } from '~/lib/api-error-presentation'

type ViewerRef = Ref<ViewerId>

function emptyState(): PublicGameState {
  return {
    enabledRuleModules: [],
    status: 'InProgress',
    turnNumber: 1,
    phase: 'TurnStart',
    currentPlayer: null,
    players: [],
    turnOrder: [],
    hp: [],
    hands: [],
    discard: [],
    playerDecks: [],
    playerDiscards: [],
    pouches: [],
    preparationPlayer: null,
    coveredPassives: [],
    counterEffects: [],
    pendingChoice: null,
    pendingRandomness: null,
    shields: [],
    statuses: [],
    jianghuStates: [],
    limitedUses: [],
    confluenceCardObligations: [],
    scheduledEchoes: [],
    flowStates: [],
    formationSuppressions: [],
    scheduledPlantEarth: [],
    environment: null,
    teamStars: [],
    starHistories: [],
    fiveStarAlignment: null,
    professions: [],
    professionCatalog: [],
    cardInterpretations: [],
    spirits: [],
    previousTurnFormation: null,
  }
}

export function useGameRoom(viewer: ViewerRef) {
  const state = ref<PublicGameState>(emptyState())
  const metadata = ref<GameRoomMetadata | null>(null)
  const invitation = ref<GameRoomInvitation | null>(null)
  const lockedDeckName = ref<string | null>(null)
  const savableReplay = ref<GameRoomResponse['savableReplay']>()
  const onlineGameId = ref<string | null>(null)
  const publicEvents = ref<PublicGameEvent[]>([])
  const selectedCards = ref<CardInstanceId[]>([])
  const selectedChoiceCards = ref<CardInstanceId[]>([])
  const playableActions = ref<PlayableAction[]>([])
  const playableAbilities = computed(() => (
    playableActions.value.filter(
      (action): action is Extract<
        PlayableAction,
        { type: 'activateProfessionAbility' | 'useSpiritSkill' }
      > => action.type === 'activateProfessionAbility' || action.type === 'useSpiritSkill',
    )
  ))
  const playableMainActions = computed(() => (
    playableActions.value.filter(
      action => action.type !== 'activateProfessionAbility' && action.type !== 'useSpiritSkill',
    )
  ))
  const errorMessage = ref<string | null>(null)
  const isLoading = ref(false)
  const interaction = ref<GameRoomResponse['interaction']>({
    canPass: false,
    hasOptionalEffect: false,
    canRetrieveDiscard: false,
    discardRetrievalAction: null,
    canChooseInitialPouch: false,
    canTriggerPouch: false,
    pouchChainAction: null,
    secretStrategyActions: [],
  })
  const connectionState = ref<'idle' | 'connecting' | 'connected' | 'reconnecting'>('idle')
  const roomDissolved = ref(false)
  let roomSocket: WebSocket | null = null
  let roomSocketGameId: string | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let reconnectAttempt = 0
  let playableQueryRevision = 0
  let playableQueryInFlight = false

  watch(viewer, () => {
    selectedCards.value = []
    selectedChoiceCards.value = []
    playableActions.value = []

    if (onlineGameId.value) {
      void refreshOnlineGame()
    }
  })

  watch(selectedCards, () => {
    const revision = ++playableQueryRevision
    if (viewer.value !== state.value.currentPlayer) {
      playableActions.value = []
      return
    }

    void queryPlayableActions(revision, [...selectedCards.value])
  })

  function canSelectCard(player: PlayerId): boolean {
    return viewer.value === player && state.value.currentPlayer === player && !state.value.pendingChoice
  }

  const canSubmitPendingChoice = computed(() => {
    const choice = state.value.pendingChoice
    return Boolean(
      choice
      && (choice.kind === 'EffectGenerated' || choice.kind === 'TypedEffect')
      && choice.cards.length > 0
      && viewer.value === choice.player
      && selectedChoiceCards.value.length >= choice.minimumCount
      && selectedChoiceCards.value.length <= choice.maximumCount,
    )
  })

  function pendingChoiceKey(choice: PublicGameState['pendingChoice']): string {
    return choice
      ? [
          choice.player,
          choice.kind,
          choice.purpose,
          choice.requiredCount,
          choice.cards.map(card => card.id).join(','),
          choice.players.join(','),
          choice.formations.join(','),
          choice.environments.join(','),
          choice.canDecline,
        ].join(':')
      : ''
  }

  function applyRoomResponse(
    response: GameRoomResponse,
    preservePlayableActionsForSameState = false,
  ) {
    const preservePlayableActions = preservePlayableActionsForSameState
      && selectedCards.value.length > 0
      && JSON.stringify(state.value) === JSON.stringify(response.state)
    if (!preservePlayableActions) {
      playableQueryRevision += 1
      if (playableQueryInFlight) {
        playableQueryInFlight = false
        isLoading.value = false
      }
    }
    const previousState = state.value
    const previousChoiceKey = pendingChoiceKey(previousState.pendingChoice)
    metadata.value = response.metadata
    invitation.value = response.invitation ?? null
    lockedDeckName.value = response.lockedDeckName ?? null
    savableReplay.value = response.savableReplay
    onlineGameId.value = response.gameId
    state.value = response.state
    const reconciledDraft = reconcileActionDraft(
      selectedCards.value,
      previousState,
      response.state,
    )
    if (reconciledDraft.length !== selectedCards.value.length) {
      selectedCards.value = reconciledDraft
    }
    publicEvents.value = response.events
    if (!preservePlayableActions) {
      playableActions.value = response.playableActions
    }
    interaction.value = response.interaction
    errorMessage.value = null
    roomDissolved.value = response.metadata.status === 'Dissolved'

    if (pendingChoiceKey(response.state.pendingChoice) !== previousChoiceKey) {
      selectedChoiceCards.value = []
    }

    if (import.meta.client) {
      connectRoomSocket(response.gameId)
    }
  }

  async function submitOnline(action: OnlineGameAction): Promise<boolean> {
    if (!onlineGameId.value) {
      return false
    }

    playableQueryInFlight = true
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
      errorMessage.value = presentApiError(error, '無法完成遊戲操作，請稍後再試。')
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
      errorMessage.value = presentApiError(error, '無法更新線上房間')
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
      errorMessage.value = presentApiError(error, '無法開始線上遊戲')
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

  async function updateRuleModules(enabledRuleModules: string[]) {
    if (!onlineGameId.value) return false

    isLoading.value = true
    errorMessage.value = null
    try {
      applyRoomResponse(await $fetch<GameRoomResponse>(
        `/api/games/${onlineGameId.value}/rules`,
        {
          method: 'PUT',
          body: { enabledRuleModules },
        },
      ))
      return true
    } catch (error) {
      errorMessage.value = presentApiError(error, '無法更新規則')
      return false
    } finally {
      isLoading.value = false
    }
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
      errorMessage.value = presentApiError(error, '房間操作失敗')
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

  async function advanceAutomatic() {
    return await submitOnline({ type: 'advanceAutomatic' })
  }

  async function retrievePreviousTurnDiscard() {
    const player = state.value.currentPlayer
    if (!player) return

    await submitOnline({
      type: 'retrievePreviousTurnDiscard',
      player,
    })
  }

  async function chooseInitialPouch(card: CardInstanceId) {
    if (viewer.value === 'observer') return
    await submitOnline({
      type: 'chooseInitialPouch',
      player: viewer.value,
      card,
    })
  }

  async function triggerSecretStrategy(
    strategy: SecretStrategy,
    options: {
      targetPlayer?: PlayerId
      star?: StarKind
      breakStar?: boolean
      discardCard?: CardInstanceId
      deckCards?: CardInstanceId[]
      discardCards?: CardInstanceId[]
    } = {},
  ) {
    if (viewer.value === 'observer') return false
    return await submitOnline({
      type: 'triggerSecretStrategy',
      player: viewer.value,
      strategy,
      ...options,
    })
  }

  async function queryPlayableActions(revision: number, cards: CardInstanceId[]) {
    const player = state.value.currentPlayer

    if (!player || !onlineGameId.value) {
      return
    }

    isLoading.value = true
    errorMessage.value = null
    try {
      const response = await $fetch<GameRoomResponse>(
        `/api/games/${onlineGameId.value}/commands`,
        {
          method: 'POST',
          body: {
            action: {
              type: 'playableActions',
              player,
              cards,
            },
          },
        },
      )
      if (revision !== playableQueryRevision) {
        return
      }
      playableActions.value = response.playableActions
      interaction.value = response.interaction
    } catch (error) {
      if (revision === playableQueryRevision) {
        errorMessage.value = presentApiError(error, '無法取得可用行動，請稍後再試。')
      }
    } finally {
      if (revision === playableQueryRevision) {
        playableQueryInFlight = false
        isLoading.value = false
      }
    }
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

    if (choice.kind === 'EffectGenerated' || choice.kind === 'TypedEffect') {
      togglePendingChoiceCard(card)
    }
  }

  function togglePendingChoiceCard(card: CardInstanceId) {
    const choice = state.value.pendingChoice

    if (
      !choice
      || (choice.kind !== 'EffectGenerated' && choice.kind !== 'TypedEffect')
      || choice.cards.length === 0
      || viewer.value !== choice.player
    ) {
      return
    }

    selectedChoiceCards.value = togglePendingChoiceSelection(
      selectedChoiceCards.value,
      card,
      choice.maximumCount,
    )
  }

  async function submitPendingChoice() {
    const choice = state.value.pendingChoice
    const cards = choice
      && (choice.kind === 'EffectGenerated' || choice.kind === 'TypedEffect')
      && selectedChoiceCards.value.length >= choice.minimumCount
      && selectedChoiceCards.value.length <= choice.maximumCount
      ? [...selectedChoiceCards.value]
      : undefined

    if (
      !choice
      || (choice.kind !== 'EffectGenerated' && choice.kind !== 'TypedEffect')
      || choice.cards.length === 0
      || viewer.value !== choice.player
      || !cards
    ) {
      return
    }

    if (await submitOnline({
      type: 'answerEffectChoiceTyped',
      player: choice.player,
      answer: { type: 'cards', cards },
    })) {
      selectedChoiceCards.value = []
    }
  }

  async function choosePendingPlayer(player: PlayerId) {
    const choice = state.value.pendingChoice
    if (
      !choice
      || choice.kind !== 'TypedEffect'
      || viewer.value !== choice.player
      || !choice.players.includes(player)
    ) {
      return
    }
    await submitOnline({
      type: 'answerEffectChoiceTyped',
      player: choice.player,
      answer: { type: 'player', player },
    })
  }

  async function choosePendingFormation(formationId: string) {
    const choice = state.value.pendingChoice
    if (
      !choice
      || choice.kind !== 'TypedEffect'
      || viewer.value !== choice.player
      || !choice.formations.includes(formationId)
    ) {
      return
    }
    await submitOnline({
      type: 'answerEffectChoiceTyped',
      player: choice.player,
      answer: { type: 'formation', formationId },
    })
  }

  async function declinePendingChoice() {
    const choice = state.value.pendingChoice
    if (
      !choice
      || choice.kind !== 'TypedEffect'
      || viewer.value !== choice.player
      || !choice.canDecline
    ) {
      return
    }
    await submitOnline({
      type: 'answerEffectChoiceTyped',
      player: choice.player,
      answer: { type: 'decline' },
    })
  }

  async function choosePendingEnvironment(environment: Element) {
    const choice = state.value.pendingChoice
    if (
      !choice
      || choice.kind !== 'TypedEffect'
      || viewer.value !== choice.player
      || !choice.environments.includes(environment)
    ) {
      return
    }
    await submitOnline({
      type: 'answerEffectChoiceTyped',
      player: choice.player,
      answer: { type: 'environment', environment },
    })
  }

  function toggleCardSelection(player: PlayerId, card: CardInstanceId) {
    if (!canSelectCard(player)) {
      return
    }

    selectedCards.value = toggleActionDraftCard(selectedCards.value, card)
  }

  async function performPlayableAction(
    action: PlayableAction,
    pouchOptions: {
      pouchOwner?: PlayerId
      pouchCard?: CardInstanceId
      triggerCard?: CardInstanceId
      secretStrategy?: SecretStrategy
      secretStrategyTargetPlayer?: PlayerId
      secretStrategyStar?: StarKind
      secretStrategyBreakStar?: boolean
      secretStrategyDiscardCard?: CardInstanceId
      secretStrategyDeckCards?: CardInstanceId[]
      secretStrategyDiscardCards?: CardInstanceId[]
    } = {},
  ): Promise<boolean> {
    const player = state.value.currentPlayer

    if (viewer.value !== player) {
      return false
    }

    switch (action.type) {
      case 'performFormation': {
        const submitted = await submitOnline({
          type: 'performFormation',
          player,
          formationId: action.id,
          cards: action.cards,
          starSubstitutionCard: action.starSubstitution?.card,
          matchOptionRole: action.matchOption?.role,
          matchOptionCard: action.matchOption?.card,
          matchOptionSlots: action.matchOption?.slots,
          ...pouchOptions,
        })
        if (submitted) {
          selectedCards.value = []
        }
        return submitted
      }
      case 'changeProfession': {
        const submitted = await submitOnline({
          type: 'changeProfession',
          player,
          professionId: action.id,
          cards: action.cards,
        })
        if (submitted) {
          selectedCards.value = []
        }
        return submitted
      }
      case 'activateProfessionAbility': {
        const submitted = await submitOnline({
          type: 'activateProfessionAbility',
          player,
          abilityId: action.id,
          cards: action.cards,
          targetCard: action.targetCard ?? undefined,
          declaredElement: action.declaredElement ?? undefined,
          declaredLevel: action.declaredLevel ?? undefined,
        })
        if (submitted) {
          selectedCards.value = []
        }
        return submitted
      }
      case 'useSpiritSkill': {
        const submitted = await submitOnline({
          type: 'useSpiritSkill',
          player,
          skill: action.id,
          selectedCard: action.selectedCard ?? undefined,
          declaredLevel: action.declaredLevel ?? undefined,
        })
        if (submitted) {
          selectedCards.value = []
        }
        return submitted
      }
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
        applyRoomResponse(message.data, true)
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
    lockedDeckName.value = null
    savableReplay.value = undefined
    onlineGameId.value = null
    state.value = emptyState()
    publicEvents.value = []
    selectedCards.value = []
    selectedChoiceCards.value = []
    playableActions.value = []
    errorMessage.value = null
    roomDissolved.value = false
  }

  onBeforeUnmount(disconnectRoomSocket)

  return {
    metadata,
    invitation,
    lockedDeckName,
    savableReplay,
    onlineGameId,
    state,
    publicEvents,
    selectedCards,
    selectedChoiceCards,
    playableActions,
    playableAbilities,
    playableMainActions,
    errorMessage,
    isLoading,
    interaction,
    canSubmitPendingChoice,
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
    updateRuleModules,
    clearRoom,
    disconnectRoomSocket,
    advanceAutomatic,
    passAction,
    retrievePreviousTurnDiscard,
    chooseInitialPouch,
    triggerSecretStrategy,
    canSelectCard,
    toggleCardSelection,
    performPlayableAction,
    choosePendingCard,
    choosePendingPlayer,
    choosePendingFormation,
    choosePendingEnvironment,
    declinePendingChoice,
    togglePendingChoiceCard,
    submitPendingChoice,
  }
}
