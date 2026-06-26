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
import type { GameRoomMetadata, GameRoomResponse, OnlineGameAction } from '../../shared/game-room'

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
  const firstPlayer = ref<PlayerId>('alice')
  const deckSeed = ref(crypto.randomUUID())

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
    errorMessage.value = null
  }

  function applyRoomResponse(response: GameRoomResponse) {
    metadata.value = response.metadata
    onlineGameId.value = response.gameId
    state.value = response.state
    publicEvents.value = response.events
    playableFormations.value = response.playableFormations
    errorMessage.value = null
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

  async function submitOnline(action: OnlineGameAction) {
    if (!onlineGameId.value) {
      return
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
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '線上房間呼叫失敗'
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
    selectedCards.value = []

    if (onlineGameId.value) {
      await submitOnline({ type: 'passAction' })
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

    selectedCards.value = []

    if (choice.kind === 'TurnDrawDiscard') {
      if (onlineGameId.value) {
        await submitOnline({
          type: 'chooseTurnDiscard',
          player: choice.player,
          card,
        })
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
      return
    }

    if (choice.kind === 'EffectGenerated') {
      if (onlineGameId.value) {
        await submitOnline({
          type: 'answerEffectChoice',
          player: choice.player,
          cards: [card],
        })
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

    if (onlineGameId.value) {
      await submitOnline({
        type: 'performFormation',
        player,
        formationId: formation.id,
        cards,
      })
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
  }

  return {
    metadata,
    onlineGameId,
    state,
    publicEvents,
    selectedCards,
    playableFormations,
    errorMessage,
    isLoading,
    startSampleGame,
    applyRoomResponse,
    refreshOnlineGame,
    startOnlineGame,
    passAction,
    advanceAutomatic,
    canSelectCard,
    toggleCardSelection,
    performFormation,
    choosePendingCard,
  }
}
