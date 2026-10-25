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
} from '#shared/game-room'
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
  shouldApplyRoomMetadataVersion,
} from '~/lib/active-game-version'
import { isInitialPouchAlreadyChosen } from '~/lib/initial-pouch-selection'
import { presentApiError } from '~/lib/api-error-presentation'
import { createSubmissionController } from '~/lib/submission-controller'
import {
  parseGameRoomCompetitionMode,
  type CompetitionRoomMode,
} from '~/lib/competition-ready'

import type { RoomOperations } from '#shared/realtime'

type ViewerRef = Ref<ViewerId>

function emptyState(): PublicGameState {
  return {
    ruleVersion: '5.16',
    totems: [],
    enabledRuleModules: [],
    status: 'InProgress',
    turnNumber: 1,
    phase: 'TurnStart',
    currentPlayer: null,
    players: [],
    turnOrder: [],
    hp: [],
    initialHp: [],
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
    terminalResolution: null,
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
  const competitionMode = ref<CompetitionRoomMode>({ kind: 'ordinary' })
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
  const roomOperations = ref(0)
  const isSubmittingCommand = ref(false)
  const isQueryingPlayableActions = ref(false)
  const isLoading = computed(() => isSubmittingCommand.value || roomOperations.value > 0
    || (isQueryingPlayableActions.value && !state.value.pendingChoice))
  const interaction = ref<GameRoomResponse['interaction']>({
    canChooseInitialPouch: false,
  })
  const roomDissolved = ref(false)
  const roomChannel = useRealtimeSocket<RoomOperations>((value, reconnected) => {
    const message = value as GameRoomSocketMessage
    if (message.type === 'roomState' && message.data.gameId === onlineGameId.value) {
      applyRoomResponse(message.data, true, reconnected)
    } else if (message.type === 'roomDissolved') {
      roomDissolved.value = true
      disconnectRoomSocket()
    }
  })
  const connectionState = roomChannel.connectionState
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
    if (viewer.value !== state.value.currentPlayer || state.value.pendingChoice) {
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
    if (!shouldApplyRoomMetadataVersion(metadata.value ?? undefined, response.metadata)) {
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
      }
    }
    const previousState = state.value
    competitionMode.value = parseGameRoomCompetitionMode(response)
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
    errorMessage.value = null

    try {
      const response = await roomChannel.request('command', {
        commandId: submission.commandId,
        gameInstanceId: submission.gameInstanceId,
        transactionId: submission.transactionId,
        action,
      })
      if (!('gameId' in response)) throw new Error('Invalid command response')
      applyRoomResponse(response)
      retryableSubmission = null
      return true
    } catch (error) {
      if ((error as { data?: { code?: string } })?.data?.code === 'socketTimeout') {
        await refreshOnlineGame()
      }
      if (action.type === 'chooseInitialPouch' && isInitialPouchAlreadyChosen(error)) {
        retryableSubmission = null
        await refreshOnlineGame()
        return true
      }
      errorMessage.value = presentApiError(error, '無法完成遊戲操作，請稍後再試。')
      return false
    } finally {
      isSubmittingCommand.value = false
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

    roomOperations.value += 1
    errorMessage.value = null

    try {
      applyRoomResponse(await roomChannel.request('state', undefined))
    } catch (error) {
      errorMessage.value = presentApiError(error, '無法更新線上房間')
    } finally {
      roomOperations.value -= 1
    }
  }

  async function startOnlineGame() {
    await roomMutation('start', undefined)
  }

  async function toggleReady() {
    const member = metadata.value?.members.find(member => member.player === viewer.value)
    if (member) await roomMutation('ready', { ready: !member.ready })
  }

  async function leaveOnlineRoom() {
    const successful = await roomMutation('leave', undefined)

    if (successful) {
      clearRoom()
    }

    return successful
  }

  async function removeOnlinePlayer(userId: string) {
    return await roomMutation('remove', { userId })
  }

  async function dissolveOnlineRoom() {
    const successful = await roomMutation('dissolve', undefined)

    if (successful) {
      disconnectRoomSocket()
      roomDissolved.value = true
    }

    return successful
  }

  async function resetOnlineRoom() {
    return await roomMutation('reset', undefined)
  }

  async function updateRuleModules(enabledRuleModules?: string[], ruleVersion?: '5.16' | '5.17') {
    return await roomMutation('rules', { enabledRuleModules, ruleVersion })
  }

  async function roomMutation<Name extends Exclude<keyof RoomOperations, 'command' | 'competitionReady' | 'clock'>>(
    operation: Name, input: RoomOperations[Name]['input'],
  ): Promise<boolean> {
    if (!onlineGameId.value) return false
    roomOperations.value += 1
    errorMessage.value = null
    try {
      const response = await roomChannel.request(operation, input)
      applyRoomResponse(response as GameRoomResponse)
      return true
    } catch (error) {
      errorMessage.value = presentApiError(error, '房間操作失敗')
      return false
    } finally {
      roomOperations.value -= 1
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
    errorMessage.value = null
    try {
      const response = await roomChannel.request('command', {
        commandId: crypto.randomUUID(), gameInstanceId,
        transactionId: activeTransactionId ?? crypto.randomUUID(),
        action: { type: 'playableActions', player, cards },
      })
      if (revision !== playableQueryRevision) {
        return
      }
      playableActions.value = response.playableActions
    } catch (error) {
      if (revision === playableQueryRevision) {
        errorMessage.value = presentApiError(error, '無法取得可用行動，請稍後再試。')
      }
    } finally {
      if (revision === playableQueryRevision) {
        playableQueryInFlight = false
        isQueryingPlayableActions.value = false
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
    if (gameId) roomChannel.connect(`/api/games/${encodeURIComponent(gameId)}/socket`)
  }

  function disconnectRoomSocket() {
    roomChannel.close()
  }

  async function setCompetitionReady(ready: boolean, options?: { signal: AbortSignal }) {
    return await roomChannel.request('competitionReady', { ready }, options)
  }

  async function readCompetitionClock(options?: { signal: AbortSignal }) {
    return await roomChannel.request('clock', undefined, options)
  }

  function clearRoom() {
    disconnectRoomSocket()
    metadata.value = null
    competitionMode.value = { kind: 'ordinary' }
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
    competitionMode,
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
    setCompetitionReady,
    readCompetitionClock,
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
