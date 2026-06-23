import type {
  PlayableFormation,
  PlayerId,
  PublicGameEvent,
  PublicGameState,
  PublicPlayerHand,
  ViewerId,
} from '~/types/fewfc'

type ViewerRef = Ref<ViewerId>

const samplePlayers = [
  { id: 'alice', team: 'team:alice' },
  { id: 'bob', team: 'team:bob' },
] as const

const canonicalHands: Record<PlayerId, string[]> = {
  alice: ['金 1', '火 2', '木 4', '水 2', '土 3'],
  bob: ['火 1', '水 5', '金 3', '土 2', '木 1'],
}

function cloneHands(): Record<PlayerId, string[]> {
  return {
    alice: [...canonicalHands.alice],
    bob: [...canonicalHands.bob],
  }
}

function publicHandsFor(viewer: ViewerId, hands: Record<PlayerId, string[]>): PublicPlayerHand[] {
  return samplePlayers.map((player) => {
    const cards = hands[player.id]

    if (viewer === player.id) {
      return {
        player: player.id,
        cards: { kind: 'known', cards },
      }
    }

    return {
      player: player.id,
      cards: { kind: 'hidden', count: cards.length },
    }
  })
}

function sampleStateFor(viewer: ViewerId, hands: Record<PlayerId, string[]>): PublicGameState {
  return {
    status: 'InProgress',
    turnNumber: 1,
    phase: 'MainPhase',
    currentPlayer: 'alice',
    players: samplePlayers.map((player) => ({ ...player })),
    turnOrder: ['alice', 'bob'],
    hp: [
      { team: 'team:alice', hp: 20 },
      { team: 'team:bob', hp: 20 },
    ],
    hands: publicHandsFor(viewer, hands),
    discard: [],
    coveredPassives: [],
    pendingChoice: null,
    shields: [],
    statuses: [],
  }
}

function initialEvents(): PublicGameEvent[] {
  return [
    {
      id: 'event-3',
      type: 'TurnStarted',
      summary: 'Alice 開始第 1 回合。',
    },
    {
      id: 'event-2',
      type: 'CardsDealt',
      summary: 'Bob 收到 5 張隱藏手牌。',
    },
    {
      id: 'event-1',
      type: 'CardsDealt',
      summary: 'Alice 收到 5 張隱藏手牌。',
    },
  ]
}

export function useLocalGame(viewer: ViewerRef) {
  const hands = ref<Record<PlayerId, string[]>>(cloneHands())
  const state = ref<PublicGameState>(sampleStateFor(viewer.value, hands.value))
  const publicEvents = ref<PublicGameEvent[]>(initialEvents())
  const selectedCards = ref<string[]>([])

  const playableFormations = computed<PlayableFormation[]>(() => {
    if (viewer.value !== state.value.currentPlayer || selectedCards.value.length === 0) {
      return []
    }

    const selected = selectedCards.value
    const formations: PlayableFormation[] = []

    if (selected.length >= 2) {
      formations.push({
        id: 'basic-attack',
        name: '合擊',
        category: 'Attack',
        summary: '以選取的牌對上一位玩家所屬隊伍造成傷害。',
      })
    }

    if (selected.some((card) => card.startsWith('火'))) {
      formations.push({
        id: 'fire-spell',
        name: '烈火術',
        category: 'Spell',
        summary: '以火元素牌發動立即法術。',
      })
    }

    if (selected.length === 5) {
      formations.push({
        id: 'five-elements-cycle',
        name: '五行流轉',
        category: 'Spell',
        summary: '五張牌齊備時發動的大型法術。',
      })
    }

    return formations
  })

  watch(viewer, (nextViewer) => {
    selectedCards.value = []
    state.value = {
      ...state.value,
      hands: publicHandsFor(nextViewer, hands.value),
    }
  })

  function prependEvent(type: string, summary: string) {
    publicEvents.value = [
      {
        id: `event-${Date.now()}`,
        type,
        summary,
      },
      ...publicEvents.value,
    ]
  }

  function startSampleGame() {
    hands.value = cloneHands()
    selectedCards.value = []
    state.value = sampleStateFor(viewer.value, hands.value)
    publicEvents.value = initialEvents()
  }

  function passAction() {
    const previousPlayer = state.value.currentPlayer
    const nextPlayer = previousPlayer === 'alice' ? 'bob' : 'alice'

    state.value = {
      ...state.value,
      currentPlayer: nextPlayer,
      turnNumber: state.value.turnNumber + 1,
      hands: publicHandsFor(viewer.value, hands.value),
      pendingChoice: {
        player: nextPlayer,
        kind: nextPlayer === viewer.value ? 'Choose one drawn card to discard' : 'Hidden',
      },
    }

    prependEvent('ActionPassed', `${previousPlayer} 跳過行動。`)
  }

  function advanceAutomatic() {
    state.value = {
      ...state.value,
      phase: state.value.pendingChoice ? 'TurnDrawDiscardChoice' : 'MainPhase',
      pendingChoice: null,
    }

    prependEvent('AutomaticAdvance', '規則自動推進到下一個決策點。')
  }

  function canSelectCard(player: PlayerId): boolean {
    return viewer.value === player && state.value.currentPlayer === player && !state.value.pendingChoice
  }

  function toggleCardSelection(player: PlayerId, card: string) {
    if (!canSelectCard(player)) {
      return
    }

    selectedCards.value = selectedCards.value.includes(card)
      ? selectedCards.value.filter((selected) => selected !== card)
      : [...selectedCards.value, card]
  }

  function performFormation(formation: PlayableFormation) {
    const player = state.value.currentPlayer

    if (viewer.value !== player || selectedCards.value.length === 0) {
      return
    }

    const usedCards = [...selectedCards.value]
    hands.value = {
      ...hands.value,
      [player]: hands.value[player].filter((card) => !usedCards.includes(card)),
    }

    const nextPlayer = player === 'alice' ? 'bob' : 'alice'
    selectedCards.value = []
    state.value = {
      ...state.value,
      currentPlayer: nextPlayer,
      turnNumber: state.value.turnNumber + 1,
      hands: publicHandsFor(viewer.value, hands.value),
      discard: [...state.value.discard, ...usedCards],
      pendingChoice: null,
    }

    prependEvent('FormationPerformed', `${player} 發動「${formation.name}」，使用 ${usedCards.length} 張牌。`)
  }

  return {
    state,
    publicEvents,
    selectedCards,
    playableFormations,
    startSampleGame,
    passAction,
    advanceAutomatic,
    canSelectCard,
    toggleCardSelection,
    performFormation,
  }
}
