export type PlayerId = string
export type TeamId = string
export type ViewerId = PlayerId | 'observer'
export type CardInstanceId = number
export type StarKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type SpiritKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth' | 'Evil' | 'Death'
export type Element = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type SecretStrategy =
  | 'GoldenCicada'
  | 'StealTheBeam'
  | 'MuddyWaters'
  | 'WatchTheFire'
  | 'LureTheTigerAway'
  | 'ReturnSoul'
  | 'SheepStealing'
  | 'DarkCrossing'
  | 'DeceiveHeaven'
  | 'Retreat'

export interface StarElementSubstitution {
  card: CardInstanceId
  printedElement: Element
  interpretedElement: Element
}

export interface PublicPlayer {
  id: PlayerId
  team: TeamId
}

export interface TeamHp {
  team: TeamId
  hp: number
}

export type PublicCardRefs =
  | {
      kind: 'known'
      cards: PublicCard[]
    }
  | {
      kind: 'hidden'
      count: number
    }
  | {
      kind: 'partiallyKnown'
      cards: Array<PublicCard | null>
    }

export interface PublicPlayerHand {
  player: PlayerId
  cards: PublicCardRefs
}

export interface PublicCoveredPassive {
  owner: PlayerId
  formationId: string | null
  cards: PublicCardRefs
  starSubstitution: StarElementSubstitution | null
}

export interface PublicPreviousTurnFormation {
  player: PlayerId
  formationId: string | null
  formationName: string | null
  cards: PublicCardRefs
}

export type PublicPendingChoice = VisiblePendingChoice | HiddenPendingChoice

export interface VisiblePendingChoice {
  visibility: 'visible'
  choiceId: number
  player: PlayerId
  reason: PendingChoicePresentation
  choice: PendingChoice
}

export interface HiddenPendingChoice {
  visibility: 'hidden'
  player: PlayerId
  reason: PendingChoicePresentation
}

export type PendingChoice =
  | {
      type: 'card'
      cards: PublicCard[]
      minimum: number
      maximum: number
      canDecline: boolean
    }
  | {
      type: 'player'
      players: PlayerId[]
      canDecline: boolean
    }
  | {
      type: 'formation'
      formations: string[]
      formationGroups: FormationChoiceGroup[]
      canDecline: boolean
    }
  | {
      type: 'environment'
      environments: Element[]
      canDecline: boolean
    }
  | {
      type: 'chain'
      pouchOwners: PlayerId[]
      deckCards: PublicCard[]
      strategyOptions: SecretStrategyOption[]
    }
  | {
      type: 'sheepStealing'
      deckCards: PublicCard[]
      discardCards: PublicCard[]
    }

export interface FormationChoiceGroup {
  ruleModuleId: string | null
  formations: Array<{
    id: string
    name: string
  }>
}

export type EchoMelodyPresentation =
  | 'ringingMetal'
  | 'fallingWood'
  | 'flowingWater'
  | 'warFire'
  | 'splitEarth'

export type PendingChoicePresentation =
  | { type: 'turnDrawDiscard' }
  | { type: 'holyWind' }
  | { type: 'chaos' }
  | { type: 'revelation' }
  | { type: 'azureCloudStep' }
  | { type: 'clearWind' }
  | { type: 'clearWindTenThousandMiles' }
  | { type: 'mirrorResonance' }
  | { type: 'myriadResonance' }
  | { type: 'thousandResonance' }
  | { type: 'echoRingingMetalDeckCard' }
  | { type: 'echoCost'; melody: EchoMelodyPresentation }
  | { type: 'echoSplitEarthFormation' }
  | { type: 'echoPureFirePlayer' }
  | { type: 'echoPlantEarthMelody' }
  | { type: 'earthRendingEnvironment' }
  | { type: 'earthRendingCard' }
  | { type: 'chain' }
  | { type: 'sheepStealing' }
  | { type: 'metamorphosis' }
  | { type: 'sealCard' }
  | { type: 'unclassified' }

export type ChoiceAnswer =
  | { type: 'cards'; cards: CardInstanceId[] }
  | { type: 'player'; player: PlayerId }
  | { type: 'formation'; formationId: string }
  | { type: 'environment'; environment: Element }
  | {
      type: 'chain'
      pouchOwner: PlayerId
      pouchCard: CardInstanceId
      triggerCard?: CardInstanceId
      strategy?: SecretStrategy
      targetPlayer?: PlayerId
      star?: StarKind
      breakStar?: boolean
      discardCard?: CardInstanceId
    }
  | { type: 'sheepStealing'; deckCards: CardInstanceId[]; discardCards: CardInstanceId[] }
  | { type: 'decline' }

export interface PublicPendingRandomness {
  requestId: string
  deck: string
  operation: 'deckShuffle' | 'discardShuffle'
  cardCount: number
}

export interface PublicCard {
  id: CardInstanceId
  label: string
  element: Element | null
  level: number | null
  secretStrategies: Array<{
    strategy: SecretStrategy
    input: 'none' | 'targetPlayer' | 'deckDiscardSwap' | 'star' | 'retreat'
  }>
}

export interface RuleModuleSpec {
  id: string
  category: 'advanced' | 'optional' | 'theme'
  defaultEnabled: boolean
  dependencies: string[]
}

export interface DeckCompositionCardDefinition {
  id: string
  name: string
  element: Element
  level: number
  sharedDeckCopies: number
  personalDeckCopyLimit: number
  preconstructedCopies: number
}

export interface RulesCatalog {
  version: 1
  ruleModules: RuleModuleSpec[]
  deckComposition: {
    cardDefinitions: DeckCompositionCardDefinition[]
    sharedDeck: {
      exactCardCount: number
    }
    personalDeck: {
      exactCardCount: number
      maximumLevelTotal: number
      preconstructed: {
        name: string
        cards: string[]
      }
    }
  }
}

export interface PersonalDeckResolution {
  candidateValidation: {
    valid: boolean
    cardCount: number
    levelTotal: number
    issues: Array<{ code: string } & Record<string, unknown>>
  } | null
  source: 'custom' | 'preconstructed'
  effective: {
    player: PlayerId
    name: string
    cards: string[]
  }
  effectiveValidation: {
    valid: boolean
    cardCount: number
    levelTotal: number
    issues: Array<{ code: string } & Record<string, unknown>>
  }
}

export interface PublicGameState {
  enabledRuleModules: string[]
  status: 'Preparing' | 'InProgress' | 'Finished'
  winnerTeam: TeamId | null
  turnNumber: number
  phase: 'TurnStart' | 'ActiveEffects' | 'Action' | 'TurnDraw' | 'TurnEnd'
  currentPlayer: PlayerId | null
  players: PublicPlayer[]
  turnOrder: PlayerId[]
  hp: TeamHp[]
  hands: PublicPlayerHand[]
  deckCount?: number
  discard: PublicCard[]
  playerDecks: Array<{ player: PlayerId; cards: PublicCardRefs }>
  playerDiscards: Array<{ player: PlayerId; cards: PublicCard[] }>
  pouches: Array<{ owner: PlayerId; card: PublicCard | null }>
  initialPouchSelection: { remainingPlayers: PlayerId[] } | null
  coveredPassives: PublicCoveredPassive[]
  counterEffects: Array<{ owner: PlayerId; effectId: string; effectName: string }>
  pendingChoice: PublicPendingChoice | null
  pendingRandomness: PublicPendingRandomness | null
  shields: Array<{ player: PlayerId; value: number }>
  statuses: Array<{
    id: string
    owner: { kind: 'player' | 'team'; id: string }
    kind: string
    presentation: StatusPresentation
    duration: StatusDurationPresentation
  }>
  jianghuStates: Array<{
    owner: PlayerId
    kind: 'ThousandBlades' | 'SnowTreading' | 'Poison'
    remainingTurns: number
    expiresOnTurn: number | null
  }>
  limitedUses: Array<{
    owner: PlayerId
    key: string
    presentation: LimitedUsePresentation
    remaining: number
    maximum: number
  }>
  confluenceCardObligations: Array<{
    owner: PlayerId
    card: CardInstanceId | null
    allowProfessionFormation: boolean
  }>
  scheduledEchoes: Array<{
    player: PlayerId
    melody: 'ringingMetal' | 'fallingWood' | 'flowingWater' | 'warFire' | 'splitEarth' | 'pureFire' | 'unclassified'
    dueTurnNumber: number
  }>
  flowStates: Array<{
    player: PlayerId
    layers: number
  }>
  formationSuppressions: Array<{
    source: PlayerId
    target: PlayerId
    formationName: string
    expiresOnTurnNumber: number
  }>
  scheduledPlantEarth: Array<{
    player: PlayerId
    dueTurnNumber: number
  }>
  environment: 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth' | null
  teamStars: Array<{ team: TeamId; star: StarKind }>
  starHistories: Array<{ player: PlayerId; stars: StarKind[] }>
  fiveStarAlignment: { player: PlayerId; team: TeamId } | null
  professions: Array<{
    player: PlayerId
    id: string
    name: string
    abilities: string[]
  }>
  professionCatalog: Array<{
    id: string
    name: string
    requirement: string
    parentName: string | null
    inheritance: string
    abilities: string[]
    formations: Array<{
      name: string
      summary: string
    }>
  }>
  cardInterpretations: CardInterpretationPresentation[]
  spirits: Array<{
    player: PlayerId
    spirit: SpiritKind
    power: number
  }>
  previousTurnFormation: PublicPreviousTurnFormation | null
}

export type StatusPresentation =
  | 'cannotAct'
  | 'cannotDraw'
  | 'divineCalculation'
  | 'galeRain'
  | 'goldenCicada'
  | 'watchFire'
  | 'lurePlayer'
  | 'lureSpirit'
  | 'spiritStoneShield'
  | 'jianghuFanBeyondHeaven'
  | 'jianghuYangAura'
  | 'jianghuDancingYang'
  | 'jianghuMeteor'
  | 'unclassified'

export type StatusDurationPresentation =
  | { type: 'untilTurnStart'; player: PlayerId }
  | { type: 'untilTurnEnd'; player: PlayerId }
  | { type: 'untilTurnEndNumber'; player: PlayerId; turnNumber: number }
  | { type: 'permanent' }

export type LimitedUsePresentation =
  | 'heavenlyResonance'
  | 'imprisoningArray'
  | 'tailwind'
  | 'voidRealm'
  | 'unclassified'

export type CardInterpretationPresentation =
  | {
      type: 'professionAbility'
      player: PlayerId
      ability: 'illusion' | 'phantasm' | 'blazingYangArt' | 'darkSpirit' | 'unclassified'
      card: PublicCard | null
      element: Element
      level: number
    }
  | {
      type: 'spiritSkill'
      player: PlayerId
      skill: 'Glimmer' | 'Splendor' | 'LegacyFireLevel' | 'Unclassified'
      card: PublicCard | null
      level: number
    }

export interface PublicGameEvent {
  id: string
  eventType: string
  title: string
  summary: string
}

export interface PlayerFacingActionDetail {
  consequences: RuleConsequence[]
}

export type ActionInputRequirement =
  | {
      type: 'virtualFormationCard'
      elements: Element[]
      levels: number[]
    }

export type ConsequenceCertainty = 'guaranteed' | 'conditional' | 'random' | 'followUp' | 'scheduled'

export type RuleConsequence =
  | { type: 'cost'; certainty: ConsequenceCertainty; cost: ActionCost }
  | { type: 'immediateEffect'; certainty: ConsequenceCertainty; effect: ImmediateEffect }
  | { type: 'followUpChoice'; certainty: ConsequenceCertainty; choice: FollowUpChoice }
  | { type: 'trustedRandomness'; certainty: ConsequenceCertainty; operation: TrustedRandomness }
  | { type: 'delayedEffect'; certainty: ConsequenceCertainty; timing: DelayedTiming; effect: DelayedEffect }
  | { type: 'ruleException'; certainty: ConsequenceCertainty; exception: RuleException }

export type ActionCost =
  | { type: 'discardSelectedCards' }
  | { type: 'spendSpiritPower'; amount: number }
  | { type: 'loseHp'; amount: number }
  | { type: 'optionalDiscardByPrintedElement'; allowedPrintedElements: Element[] }
  | { type: 'consumePouch' }

export type ImmediateEffect =
  | { type: 'attack'; target: ActionTarget; category: ActionAttackCategory; points: EffectAmount }
  | { type: 'resolveFormationEffect'; effect: FormationEffect }
  | { type: 'activateProfessionAbility'; effect: ProfessionAbilityEffect }
  | { type: 'useSpiritSkill'; effect: SpiritSkillEffect }
  | { type: 'triggerSecretStrategy'; effect: SecretStrategyEffect }
  | { type: 'movePreviousTurnDiscardToDeckTop' }

export type ActionTarget = 'selfPlayer' | 'selfTeam' | 'previousPlayer' | 'previousTeam' | 'nextPlayer' | 'nextTeam' | 'selectedPlayer' | 'allPlayers' | 'otherPlayers' | 'eachTeam'
export type ActionAttackCategory = 'elemental' | 'physical' | 'special'
export type EffectAmount = { type: 'fixed'; value: number } | { type: 'formula'; formula: EffectFormula }
export type EffectFormula =
  | { type: 'levelPlus'; amount: number }
  | { type: 'levelSumTimes'; multiplier: number }
  | { type: 'targetHandCountTimes'; multiplier: number }
  | { type: 'elementProductTimes'; element: Element; multiplier: number }
export type FormationEffect =
  | { type: 'coverCounter' }
  | { type: 'copyPreviousTurnFormation' }
  | { type: 'recoverHp' }
  | { type: 'reduceShield' }
  | { type: 'inspectHand' }
  | { type: 'createShield' }
  | { type: 'returnTeamHp' }
  | { type: 'drawCards' }
  | { type: 'swapTeamHp' }
  | { type: 'summonSpirit' }
  | { type: 'clearEnvironment' }
  | { type: 'damagePreviousTeamByLevelSumTimes'; multiplier: number }
  | { type: 'halvePreviousTeamHp' }
  | { type: 'damageNextTeamAndTakeHighestLevelHandCard'; damage: number }
  | { type: 'performResidualAndSelectedResonance' }
  | { type: 'performAllFiveResonanceEffects' }
  | { type: 'preventOtherPlayersFromActingOrDrawing'; durationTurns: number }
  | { type: 'gainDivineCalculationProtection' }
  | { type: 'poisonNextPlayer'; durationTurns: number }
  | { type: 'damageNextTeamAndPoisonNextPlayer'; damage: number; durationTurns: number }
  | { type: 'winIfNextTeamHpAtMost'; hpThreshold: number }
  | { type: 'changeEnvironment' }
  | { type: 'breakProfession' }
  | { type: 'limitedUseRecovery' }
  | { type: 'resolveMelodyMainEffect' }
  | { type: 'beginChainChoice' }
  | { type: 'shatterSpirits' }
  | { type: 'breakStars' }
  | { type: 'damageEachTeamBy15' }
  | { type: 'applyGaleRain' }
  | { type: 'reduceEveryShieldBy20' }
  | { type: 'attackIncreasesTo80IfShieldReduced' }
  | { type: 'chooseEnvironmentAndRequireMatchingCardOrRevealHand' }
  | { type: 'revealTopEightDiscardLevelThreeOrHigherThenShuffle' }
  | { type: 'transferEnvironmentToUsedElement' }
export type SecretStrategyEffect = 'protectTriggeringPlayer' | 'increaseHandLevels' | 'increaseTurnDraw' | 'negateNextPlayerFormationHpChanges' | 'suppressPlayerAbilitiesAndSpiritPower' | 'summonSpiritFromPouch' | 'swapDeckAndDiscard' | 'directProfessionChange' | 'breakOrGainStar' | 'clearOrChangeEnvironment'
export type ProfessionAbilityEffect =
  | { type: 'damagePreviousTeamByCardLevelTimes'; multiplier: number }
  | { type: 'increaseTurnDraw'; amount: number }
  | { type: 'drawThreeThenChooseOne' }
  | { type: 'createVirtualFormationCard'; scope: VirtualFormationScope }
  | { type: 'applyYangAura' }
  | { type: 'prepareFormationDrawBonus' }
  | { type: 'prepareCardWithLevelBonus'; amount: number; maximum: number }
  | { type: 'prepareMeteorEffect' }
  | { type: 'drawTwoThenReturnOne' }
  | { type: 'retrievePreviousPlayerDiscardForProfessionUse' }
  | { type: 'retrievePreviousPlayerDiscard' }
  | { type: 'revealDeckTopAndChooseDiscard' }
  | { type: 'applyShuffleRecovery' }
  | { type: 'prepareCardAtDeclaredLevel' }
export type VirtualFormationScope = 'elementalStrike' | 'baseFormation' | 'anyFormation'
export type SpiritSkillEffect =
  | { type: 'damagePreviousTeam'; amount: number }
  | { type: 'recoverOwnTeam'; amount: number }
  | { type: 'discardSelectedCardAndIncreaseTurnDraw'; amount: number }
  | { type: 'increaseTurnDraw'; amount: number }
  | { type: 'interpretSelectedCardLevel' }
  | { type: 'protectNextPlayerFromAttack' }
  | { type: 'setOwnShield'; amount: number }
  | { type: 'inspectRandomNextPlayerHandCards'; count: number }
  | { type: 'discardNextPlayerDeckAndDamageByHighestLevel'; count: number; multiplier: number }
export type FollowUpChoice =
  | { type: 'selectPlayer' }
  | { type: 'selectFormation' }
  | { type: 'selectDeckCard' }
  | { type: 'selectEnvironment' }
  | { type: 'selectPouchOwnerAndOptionalStrategy' }
  | { type: 'selectCards'; minimum: number; maximum: number }
export type TrustedRandomness =
  | { type: 'shuffleDeck' }
  | { type: 'shuffleDiscardIntoDeck' }
  | { type: 'selectHiddenHandCards'; count: number }
export type DelayedTiming = 'nextTurnStart' | 'nextPlayerTurn'
export type DelayedEffect = 'repeatMelodyMainEffect' | 'selectAndPerformMelodyMainEffect'
export type RuleException =
  | { type: 'ignoresOtherFormationEffects' }
  | { type: 'limitedUse'; key: string; remaining: number; maximum: number }

export type PlayableAction =
  | {
      type: 'performFormation'
      commandRole: 'action'
      id: string
      name: string
      category: 'Attack' | 'Spell'
      detail: PlayerFacingActionDetail | null
      cards: CardInstanceId[]
      starSubstitution: StarElementSubstitution | null
      matchOption: {
        role: string
        card: CardInstanceId
        slots: number
        preview: string | null
      } | null
    }
  | {
      type: 'changeProfession'
      commandRole: 'action'
      id: string
      name: string
      detail: PlayerFacingActionDetail | null
      cards: CardInstanceId[]
    }
  | {
      type: 'activateProfessionAbility'
      commandRole: 'activeEffect'
      id: string
      name: string
      detail: PlayerFacingActionDetail | null
      cards: CardInstanceId[]
      targetCard: CardInstanceId | null
      declaredElement: Element | null
      declaredLevel: number | null
      inputRequirement: ActionInputRequirement | null
    }
  | {
      type: 'useSpiritSkill'
      commandRole: 'activeEffect'
      id: string
      name: string
      detail: PlayerFacingActionDetail | null
      cards: CardInstanceId[]
      selectedCard: CardInstanceId | null
      declaredLevel: number | null
    }
  | ({
      type: 'triggerSecretStrategy'
      commandRole: 'activeEffect'
    } & SecretStrategyOption)
  | {
      type: 'retrievePreviousTurnDiscard'
      commandRole: 'activeEffect'
      detail: PlayerFacingActionDetail | null
    }
  | {
      type: 'pass'
      commandRole: 'action'
      detail: null
      reason: PassActionReason
    }

export type PassActionReason = 'NoCardsInHand' | 'CannotActByStatus'

export type RecordedDecision = unknown

export interface SecretStrategyOption {
  sourceCard: CardInstanceId
  strategy: SecretStrategy
  input: 'none' | 'targetPlayer' | 'deckDiscardSwap' | 'star' | 'retreat'
  targetPlayers: PlayerId[]
  stars: StarKind[]
  breakStars: StarKind[]
  deckCards: CardInstanceId[]
  discardCards: CardInstanceId[]
  handCards: CardInstanceId[]
  requiredCardCount: number
  detail: PlayerFacingActionDetail
}

export interface LocalGameResponse {
  record: RecordedDecision[]
  state: PublicGameState
  events: PublicGameEvent[]
  playableActions: PlayableAction[]
  interaction: {
    canChooseInitialPouch: boolean
  }
  trustedRandomCandidates?: CardInstanceId[]
  trustedRandomCandidateCount?: number
}
