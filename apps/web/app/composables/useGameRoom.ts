import type {
  CardInstanceId,
  ChoiceAnswer,
  Element,
  PlayableAction,
  PlayerId,
  BattleRecord,
  PublicGameState,
  SecretStrategyDecision,
  ViewerId,
} from '~/types/fewfc'
import type {
  GameRoomMetadata,
  GameRoomInvitation,
  GameRoomResponse,
  GameRoomSocketMessage,
  OnlineGameAction,
} from '../../shared/game-room'
import type { CardsChoiceAnswer, VisibleCardPendingChoice } from '~/lib/pending-choice-interaction'
import {
  cardChoiceAnswer,
  cardChoiceIsComplete,
  clearWindDiscardAnswer,
  declineChoiceAnswer,
  environmentChoiceAnswer,
  formationChoiceAnswer,
  immediateCardChoiceAnswer,
  isImmediateCardChoice,
  playerChoiceAnswer,
  sheepStealingChoiceAnswer,
  shouldResetPendingChoiceDraft,
  toggleChoiceCard,
  visibleCardPendingChoice,
  visiblePendingChoice,
} from '~/lib/pending-choice-interaction'
import { reconcileActionDraft, toggleActionDraftCard } from '~/lib/action-draft'
import {
  nextHighestActiveGameVersion,
  shouldApplyActiveGameVersion,
} from '~/lib/active-game-version'
import { isInitialPouchAlreadyChosen } from '~/lib/initial-pouch-selection'
import { presentApiError } from '~/lib/api-error-presentation'
import { createSubmissionController } from '~/lib/submission-controller'

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
    deckCount: 0,
    discard: [],
    playerDecks: [],
    playerDiscards: [],
    pouches: [],
    initialPouchSelection: null,
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
    winnerTeam: null,
    gameConclusion: null,
    professions: [],
    professionCatalog: [],
    cardInterpretations: [],
    spirits: [],
    previousTurnFormation: null,
    lastCompletedTurnDiscards: [],
  }
}

export function useGameRoom(viewer: ViewerRef) {
  const state = ref<PublicGameState>(emptyState())
  const metadata = ref<GameRoomMetadata | null>(null)
  const invitation = ref<GameRoomInvitation | null>(null)
  const lockedDeckName = ref<string | null>(null)
  const savableReplay = ref<GameRoomResponse['savableReplay']>()
  const onlineGameId = ref<string | null>(null)
  const battleRecord = ref<BattleRecord>({ preparation: { entries: [] }, turns: [] })
  const selectedCards = ref<CardInstanceId[]>([])
  const selectedChoiceCards = ref<CardInstanceId[]>([])
  const pendingChoiceDraftEpoch = ref(0)
  const playableActions = ref<PlayableAction[]>([])
  const playableAbilities = computed(() => (
    playableActions.value.filter(
      (action): action is Extract<
        PlayableAction,
        { type: 'activateProfessionAbility' | 'useSpiritSkill' }
      > => action.commandRole === 'activeEffect'
        && (action.type === 'activateProfessionAbility' || action.type === 'useSpiritSkill'),
    )
  ))
  const playableMainActions = computed(() => (
    playableActions.value.filter(
      action => action.commandRole === 'action' && action.type !== 'pass',
    )
  ))
  const playablePass = computed(() => (
    playableActions.value.find(
      (action): action is Extract<PlayableAction, { type: 'pass' }> => action.type === 'pass',
    ) ?? null
  ))
  const playableDiscardRetrieval = computed(() => (
    playableActions.value.find(
      (action): action is Extract<PlayableAction, { type: 'retrievePreviousTurnDiscard' }> => (
        action.type === 'retrievePreviousTurnDiscard'
      ),
    ) ?? null
  ))
  const playableSecretStrategies = computed(() => (
    playableActions.value.flatMap(action => (
      action.type === 'triggerSecretStrategy' ? [action.option] : []
    ))
  ))
  const errorMessage = ref<string | null>(null)
  const isLoading = ref(false)
  const isSubmittingCommand = ref(false)
  const isQueryingPlayableActions = ref(false)
  const interaction = ref<GameRoomResponse['interaction']>({
    canChooseInitialPouch: false,
  })
  const connectionState = ref<'idle' | 'connecting' | 'connected' | 'reconnecting'>('idle')
  const roomDissolved = ref(false)
  let roomSocket: WebSocket | null = null
  let roomSocketGameId: string | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let reconnectAttempt = 0
  let playableQueryRevision = 0
  let playableQueryInFlight = false
  let activeTransactionId: string | null = null
  let highestAppliedActiveGameVersion: GameRoomResponse['activeGameVersion']
  let retryableSubmission: {
    actionIdentity: string
    gameInstanceId: string
    commandId: string
    transactionId: string
  } | null = null

  watch(viewer, () => {
    selectedCards.value = []
    selectedChoiceCards.value = []
    pendingChoiceDraftEpoch.value += 1
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
    const choice = visiblePendingChoice(state.value.pendingChoice)
    return Boolean(
      choice
      && choice.choice.type === 'card'
      && !isImmediateCardChoice(choice.choice)
      && viewer.value === choice.player
      && cardChoiceIsComplete(choice.choice, selectedChoiceCards.value),
    )
  })

  function applyRoomResponse(
    response: GameRoomResponse,
    preservePlayableActionsForSameState = false,
    resetPendingChoiceDraft = false,
  ) {
    const version = response.activeGameVersion
    if (!shouldApplyActiveGameVersion(highestAppliedActiveGameVersion, version)) {
      return
    }
    highestAppliedActiveGameVersion = nextHighestActiveGameVersion(
      highestAppliedActiveGameVersion,
      version,
    )
    const preservePlayableActions = preservePlayableActionsForSameState
      && selectedCards.value.length > 0
      && JSON.stringify(state.value) === JSON.stringify(response.state)
    if (!preservePlayableActions) {
      playableQueryRevision += 1
      if (playableQueryInFlight) {
        playableQueryInFlight = false
        isQueryingPlayableActions.value = false
        isLoading.value = false
      }
    }
    const previousState = state.value
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
    battleRecord.value = response.battleRecord
    if (!preservePlayableActions) {
      playableActions.value = response.playableActions
    }
    interaction.value = response.interaction
    errorMessage.value = null
    roomDissolved.value = response.metadata.status === 'Dissolved'
    activeTransactionId = response.activeTransactionId ?? null

    if (shouldResetPendingChoiceDraft(
      previousState.pendingChoice,
      response.state.pendingChoice,
      resetPendingChoiceDraft,
    )) {
      selectedChoiceCards.value = []
      pendingChoiceDraftEpoch.value += 1
    }

    if (import.meta.client) {
      connectRoomSocket(response.gameId)
    }
  }

  const commandSubmission = createSubmissionController(async (action: OnlineGameAction) => {
    const gameInstanceId = metadata.value?.gameInstanceId
    if (!onlineGameId.value || !gameInstanceId) {
      return false
    }

    const actionIdentity = JSON.stringify(action)
    const submission = retryableSubmission
      && retryableSubmission.actionIdentity === actionIdentity
      && retryableSubmission.gameInstanceId === gameInstanceId
      ? retryableSubmission
      : {
          actionIdentity,
          gameInstanceId,
          commandId: crypto.randomUUID(),
          transactionId: activeTransactionId ?? crypto.randomUUID(),
        }
    retryableSubmission = submission

    isSubmittingCommand.value = true
    isLoading.value = true
    errorMessage.value = null

    try {
      applyRoomResponse(await $fetch<GameRoomResponse>(`/api/games/${onlineGameId.value}/commands`, {
        method: 'POST',
        body: {
          commandId: submission.commandId,
          gameInstanceId: submission.gameInstanceId,
          transactionId: submission.transactionId,
          action,
        },
      }))
      retryableSubmission = null
      return true
    } catch (error) {
      if (action.type === 'chooseInitialPouch' && isInitialPouchAlreadyChosen(error)) {
        retryableSubmission = null
        await refreshOnlineGame()
        return true
      }
      errorMessage.value = presentApiError(error, '無法完成遊戲操作，請稍後再試。')
      return false
    } finally {
      isSubmittingCommand.value = false
      isLoading.value = false
    }
  })

  async function submitOnline(action: OnlineGameAction): Promise<boolean> {
    if (commandSubmission.isSubmitting) return false
    return await commandSubmission.submit(action)
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

  async function advanceAutomatic() {
    return await submitOnline({ type: 'advanceAutomatic' })
  }

  async function chooseInitialPouch(card: CardInstanceId) {
    if (viewer.value === 'observer') return
    await submitOnline({
      type: 'chooseInitialPouch',
      player: viewer.value,
      card,
    })
  }

  async function triggerSecretStrategy(decision: SecretStrategyDecision) {
    if (viewer.value === 'observer') return false
    return await submitOnline({
      type: 'triggerSecretStrategy',
      player: viewer.value,
      decision,
    })
  }

  async function answerChainChoice(answer: Extract<ChoiceAnswer, { type: 'chain' }>) {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (!choice || choice.choice.type !== 'chain' || viewer.value !== choice.player) return false
    return await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer,
    })
  }

  async function answerSheepStealingChoice(deckCards: CardInstanceId[], discardCards: CardInstanceId[]) {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (!choice || choice.choice.type !== 'sheepStealing' || viewer.value !== choice.player) return false
    return await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer: sheepStealingChoiceAnswer(deckCards, discardCards),
    })
  }

  async function queryPlayableActions(revision: number, cards: CardInstanceId[]) {
    const player = state.value.currentPlayer
    const gameInstanceId = metadata.value?.gameInstanceId

    if (!player || !onlineGameId.value || !gameInstanceId) {
      return
    }

    playableQueryInFlight = true
    isQueryingPlayableActions.value = true
    isLoading.value = true
    errorMessage.value = null
    try {
      const response = await $fetch<GameRoomResponse>(
        `/api/games/${onlineGameId.value}/commands`,
        {
          method: 'POST',
          body: {
            commandId: crypto.randomUUID(),
            gameInstanceId,
            transactionId: activeTransactionId ?? crypto.randomUUID(),
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
        isQueryingPlayableActions.value = false
        isLoading.value = false
      }
    }
  }

  async function choosePendingCard(card: CardInstanceId) {
    const choice = visibleCardPendingChoice(state.value.pendingChoice)

    if (!choice || viewer.value !== choice.player) {
      return
    }
    const selectedCard = choice.choice.cards.find(candidate => candidate.id === card)
    if (!selectedCard) {
      return
    }

    const answer = immediateCardChoiceAnswer(choice, selectedCard)
    if (answer) {
      await submitCardChoiceAnswer(choice, answer)
      return
    }

    togglePendingChoiceCard(card)
  }

  function togglePendingChoiceCard(card: CardInstanceId) {
    const choice = visibleCardPendingChoice(state.value.pendingChoice)

    if (
      !choice
      || isImmediateCardChoice(choice.choice)
      || viewer.value !== choice.player
    ) {
      return
    }

    selectedChoiceCards.value = toggleChoiceCard(
      selectedChoiceCards.value,
      card,
      choice.choice.maximum,
    )
  }

  async function submitPendingChoice() {
    const choice = visibleCardPendingChoice(state.value.pendingChoice)
    const answer = choice
      && !isImmediateCardChoice(choice.choice)
      ? cardChoiceAnswer(choice.choice, [...selectedChoiceCards.value])
      : undefined

    if (
      !choice
      || viewer.value !== choice.player
      || !answer
    ) {
      return
    }

    if (await submitCardChoiceAnswer(choice, answer)) {
      selectedChoiceCards.value = []
    }
  }

  async function discardClearWindCard() {
    const choice = visibleCardPendingChoice(state.value.pendingChoice)
    if (!choice || viewer.value !== choice.player) {
      return
    }

    const answer = clearWindDiscardAnswer(choice)
    if (answer) {
      await submitCardChoiceAnswer(choice, answer)
    }
  }

  async function submitCardChoiceAnswer(
    choice: VisibleCardPendingChoice,
    answer: CardsChoiceAnswer,
  ): Promise<boolean> {
    return await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer,
    })
  }

  async function choosePendingPlayer(player: PlayerId) {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (
      !choice
      || choice.choice.type !== 'player'
      || viewer.value !== choice.player
      || !choice.choice.players.includes(player)
    ) {
      return
    }
    await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer: playerChoiceAnswer(player),
    })
  }

  async function choosePendingFormation(formationId: string) {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (
      !choice
      || choice.choice.type !== 'formation'
      || viewer.value !== choice.player
      || !choice.choice.formations.includes(formationId)
    ) {
      return
    }
    await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer: formationChoiceAnswer(formationId),
    })
  }

  async function declinePendingChoice() {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (
      !choice
      || !('canDecline' in choice.choice)
      || viewer.value !== choice.player
      || !choice.choice.canDecline
    ) {
      return
    }
    await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer: declineChoiceAnswer(),
    })
  }

  async function choosePendingEnvironment(environment: Element) {
    const choice = visiblePendingChoice(state.value.pendingChoice)
    if (
      !choice
      || choice.choice.type !== 'environment'
      || viewer.value !== choice.player
      || !choice.choice.environments.includes(environment)
    ) {
      return
    }
    await submitOnline({
      type: 'answerChoice',
      player: choice.player,
      choiceId: choice.choiceId,
      answer: environmentChoiceAnswer(environment),
    })
  }

  function toggleCardSelection(player: PlayerId, card: CardInstanceId) {
    if (!canSelectCard(player)) {
      return
    }

    selectedCards.value = toggleActionDraftCard(selectedCards.value, card)
  }

  async function performPlayableAction(action: PlayableAction): Promise<boolean> {
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
      case 'retrievePreviousTurnDiscard': {
        return await submitOnline({
          type: 'retrievePreviousTurnDiscard',
          player,
        })
      }
      case 'pass': {
        const submitted = await submitOnline({
          type: 'passAction',
          reason: action.reason,
        })
        if (submitted) {
          selectedCards.value = []
        }
        return submitted
      }
      case 'triggerSecretStrategy':
        return false
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

    const connectedAfterReconnect = reconnectAttempt > 0

    socket.addEventListener('message', (event) => {
      if (typeof event.data !== 'string' || event.data === 'pong') {
        return
      }

      const message = JSON.parse(event.data) as GameRoomSocketMessage

      if (message.type === 'roomState') {
        applyRoomResponse(message.data, true, connectedAfterReconnect)
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
    battleRecord.value = { preparation: { entries: [] }, turns: [] }
    selectedCards.value = []
    selectedChoiceCards.value = []
    pendingChoiceDraftEpoch.value += 1
    playableActions.value = []
    errorMessage.value = null
    isLoading.value = false
    isSubmittingCommand.value = false
    isQueryingPlayableActions.value = false
    playableQueryInFlight = false
    activeTransactionId = null
    highestAppliedActiveGameVersion = undefined
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
    battleRecord,
    selectedCards,
    selectedChoiceCards,
    pendingChoiceDraftEpoch,
    playableActions,
    playableAbilities,
    playableMainActions,
    playablePass,
    playableDiscardRetrieval,
    playableSecretStrategies,
    errorMessage,
    isLoading,
    isSubmittingCommand,
    isQueryingPlayableActions,
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
    chooseInitialPouch,
    triggerSecretStrategy,
    answerChainChoice,
    answerSheepStealingChoice,
    canSelectCard,
    toggleCardSelection,
    performPlayableAction,
    choosePendingCard,
    choosePendingPlayer,
    choosePendingFormation,
    choosePendingEnvironment,
    declinePendingChoice,
    discardClearWindCard,
    togglePendingChoiceCard,
    submitPendingChoice,
  }
}
