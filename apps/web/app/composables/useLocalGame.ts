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
  }
}

async function callLocalGame(body: unknown): Promise<LocalGameResponse> {
  return await $fetch<LocalGameResponse>('/api/local-game', {
    method: 'POST',
    body,
  })
}

export function useLocalGame(viewer: ViewerRef) {
  const record = ref<RecordedDecision[]>([])
  const state = ref<PublicGameState>(emptyState())
  const publicEvents = ref<PublicGameEvent[]>([])
  const selectedCards = ref<CardInstanceId[]>([])
  const playableFormations = ref<PlayableFormation[]>([])
  const errorMessage = ref<string | null>(null)
  const isLoading = ref(false)

  watch(viewer, (nextViewer) => {
    selectedCards.value = []
    playableFormations.value = []

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
    record.value = response.record
    state.value = response.state
    publicEvents.value = response.events
    playableFormations.value = response.playableFormations
    errorMessage.value = null
  }

  async function submit(body: unknown) {
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

  async function refresh(nextViewer = viewer.value) {
    await submit({
      action: { type: 'refresh' },
      viewer: nextViewer,
      record: record.value,
    })
  }

  async function startSampleGame() {
    selectedCards.value = []
    await submit({
      action: { type: 'start' },
      viewer: viewer.value,
    })
  }

  async function passAction() {
    selectedCards.value = []
    await submit({
      action: { type: 'passAction' },
      viewer: viewer.value,
      record: record.value,
    })
  }

  async function advanceAutomatic() {
    selectedCards.value = []
    await submit({
      action: { type: 'advanceAutomatic' },
      viewer: viewer.value,
      record: record.value,
    })
  }

  async function queryPlayableFormations() {
    const player = state.value.currentPlayer

    if (!player) {
      return
    }

    await submit({
      action: {
        type: 'playableFormations',
        player,
        cards: selectedCards.value,
      },
      viewer: viewer.value,
      record: record.value,
    })
  }

  async function choosePendingCard(card: CardInstanceId) {
    const choice = state.value.pendingChoice

    if (!choice || viewer.value !== choice.player) {
      return
    }

    selectedCards.value = []

    if (choice.kind === 'TurnDrawDiscard') {
      await submit({
        action: {
          type: 'chooseTurnDiscard',
          player: choice.player,
          card,
        },
        viewer: viewer.value,
        record: record.value,
      })
      return
    }

    if (choice.kind === 'EffectGenerated') {
      await submit({
        action: {
          type: 'answerEffectChoice',
          player: choice.player,
          cards: [card],
        },
        viewer: viewer.value,
        record: record.value,
      })
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
    selectedCards.value = []

    await submit({
      action: {
        type: 'performFormation',
        player,
        formationId: formation.id,
        cards,
      },
      viewer: viewer.value,
      record: record.value,
    })
  }

  onMounted(() => {
    void startSampleGame()
  })

  return {
    state,
    publicEvents,
    selectedCards,
    playableFormations,
    errorMessage,
    isLoading,
    startSampleGame,
    passAction,
    advanceAutomatic,
    canSelectCard,
    toggleCardSelection,
    performFormation,
    choosePendingCard,
  }
}
