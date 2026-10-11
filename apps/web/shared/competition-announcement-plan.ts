import type {
  AnnouncedStageQualification,
  AnnouncedTournamentStage,
  CompetitionAnnouncementPlan,
  SwissStandingPolicy,
  SwissTieBreaker,
  StageQualificationSlot,
  TournamentFormat,
} from './competition'

const MAX_U32 = 0xffff_ffff

export interface CompetitionAnnouncementStageDraft {
  stageId: string
  format: '' | TournamentFormat
  matchGames: string
  mainMinutes: string
  byoyomiSeconds: string
  swissRounds: string
  swissMatchPoints: { win: string, draw: string, loss: string, bye: string }
  swissPolicy: {
    drawCredit: { numerator: string, denominator: string }
    opponentWinRateFloorPercent: { mode: '' | 'none' | 'value', value: string }
    opponentWinRateCapPercent: { mode: '' | 'none' | 'value', value: string }
    excludeByesFromOpponentWinRate: '' | 'yes' | 'no'
    tieMode: '' | 'none' | 'configured'
    tieBreakers: ('' | SwissTieBreaker)[]
  }
  sourceSlots: {
    sourceStageId: string
    sourceType: '' | 'finishedStageWinner' | 'uniqueStandingRank'
    rank: string
    destinationSeed: string
  }[]
}

export function buildCompetitionAnnouncementPlan(
  drafts: readonly CompetitionAnnouncementStageDraft[],
): CompetitionAnnouncementPlan {
  if (!Array.isArray(drafts) || drafts.length === 0) invalid('公告至少要有一個賽事階段。')
  const stageIds = new Set<string>()
  const stages: AnnouncedTournamentStage[] = drafts.map((draft, index) => {
    const stageId = normalizedId(draft.stageId)
    if (!stageId || stageIds.has(stageId)) invalid('階段識別碼不可空白或重複。')
    stageIds.add(stageId)
    if (!isFormat(draft.format)) invalid(`第 ${index + 1} 階段請選擇賽制。`)

    const matchCount = positiveInteger(draft.matchGames, `第 ${index + 1} 階段交手格式`)
    if (matchCount > 65_535) invalid('交手局數或勝局門檻不可超過 65535。')
    const mainMinutes = nonNegativeInteger(draft.mainMinutes, `第 ${index + 1} 階段主時間`)
    const mainTimeMs = mainMinutes * 60_000
    if (!Number.isSafeInteger(mainTimeMs)) invalid('主時間超出可設定範圍。')
    const byoyomiSeconds = draft.byoyomiSeconds
    if (byoyomiSeconds !== '30' && byoyomiSeconds !== '60' && byoyomiSeconds !== '180') {
      invalid('讀秒只可選 30 秒、1 分鐘或 3 分鐘。')
    }

    const swiss = draft.format === 'swiss'
    const swissRounds = swiss ? positiveInteger(draft.swissRounds, `第 ${index + 1} 階段瑞士輪數`) : null
    const matchPoints = swiss ? {
      win: u32(draft.swissMatchPoints.win, '瑞士勝場積分'),
      draw: u32(draft.swissMatchPoints.draw, '瑞士和局積分'),
      loss: u32(draft.swissMatchPoints.loss, '瑞士敗場積分'),
      bye: u32(draft.swissMatchPoints.bye, '瑞士輪空積分'),
    } : null
    const swissPolicy: SwissStandingPolicy | null = swiss ? buildSwissPolicy(draft, matchPoints!) : null
    if (swissRounds !== null && swissRounds > MAX_U32) invalid('瑞士輪數超出可設定範圍。')

    const qualification: AnnouncedStageQualification = index === 0
      ? { type: 'allEligibleTeams' }
      : { type: 'sourceSlots', slots: parseSourceSlots(draft, drafts, index) }

    return {
      stageId,
      format: draft.format,
      matchFormat: {
        type: draft.format === 'singleElimination' || draft.format === 'doubleElimination'
          ? 'firstToWins' : 'fixedGames',
        value: matchCount,
      },
      clockConfig: {
        mainTimeMs,
        actionTimeMs: Number(byoyomiSeconds) * 1_000,
        outOfTurnChoiceTimeMs: Number(byoyomiSeconds) * 1_000,
      },
      swissRounds,
      swissPolicy,
      qualification,
    }
  })

  const plan: CompetitionAnnouncementPlan = {
    initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
    swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
    rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
    exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
    stages,
  }
  const parsed = parseCompetitionAnnouncementPlan(plan)
  if (!parsed) invalid('賽事方案格式不完整，請檢查各階段與晉級來源。')
  return parsed
}

/** HTTP、公開讀取及保存回放共用的 exact 公告方案解析器。 */
export function parseCompetitionAnnouncementPlan(input: unknown): CompetitionAnnouncementPlan | null {
  const plan = exactRecord(input, [
    'initialSeedPolicy', 'swissPairingPolicy', 'rankingPolicy', 'exactRankingTiePolicy', 'stages',
  ])
  if (!plan
    || plan.initialSeedPolicy !== 'randomDrawAfterRegistrationDeadline'
    || plan.swissPairingPolicy !== 'publishedRulesWithPersistedRandomSeed'
    || plan.rankingPolicy !== 'matchPointsThenOpponentWinsThenOpponentLosses'
    || plan.exactRankingTiePolicy !== 'organizerOrUnconflictedRefereeWithPublicReason'
    || !Array.isArray(plan.stages) || plan.stages.length === 0) return null

  const stageIds = new Set<string>()
  const stages: AnnouncedTournamentStage[] = []
  for (let index = 0; index < plan.stages.length; index += 1) {
    const value = exactRecord(plan.stages[index], [
      'stageId', 'format', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy', 'qualification',
    ])
    if (!value || typeof value.stageId !== 'string' || normalizedId(value.stageId) !== value.stageId
      || stageIds.has(value.stageId) || !isFormat(value.format)) return null
    stageIds.add(value.stageId)

    const matchFormat = exactRecord(value.matchFormat, ['type', 'value'])
    const expectedMatchType = value.format === 'singleElimination' || value.format === 'doubleElimination'
      ? 'firstToWins' : 'fixedGames'
    if (!matchFormat || matchFormat.type !== expectedMatchType || !positiveIntegerValue(matchFormat.value)
      || matchFormat.value > 65_535) return null

    const clock = exactRecord(value.clockConfig, ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'])
    if (!clock || !nonNegativeIntegerValue(clock.mainTimeMs)
      || (clock.mainTimeMs as number) % 60_000 !== 0
      || ![30_000, 60_000, 180_000].includes(clock.actionTimeMs as number)
      || clock.actionTimeMs !== clock.outOfTurnChoiceTimeMs) return null

    const swiss = value.format === 'swiss'
    let swissRounds: number | null = null
    let swissPolicy: SwissStandingPolicy | null = null
    if (swiss) {
      if (!u32PositiveIntegerValue(value.swissRounds)) return null
      const parsedPolicy = parseSwissPolicy(value.swissPolicy)
      if (!parsedPolicy) return null
      swissRounds = value.swissRounds as number
      swissPolicy = parsedPolicy
    } else if (value.swissRounds !== null || value.swissPolicy !== null) return null

    const qualification = parseQualification(value.qualification, index, stages)
    if (!qualification) return null
    stages.push({
      stageId: value.stageId,
      format: value.format,
      matchFormat: { type: expectedMatchType, value: matchFormat.value as number },
      clockConfig: {
        mainTimeMs: clock.mainTimeMs as number,
        actionTimeMs: clock.actionTimeMs as number,
        outOfTurnChoiceTimeMs: clock.outOfTurnChoiceTimeMs as number,
      },
      swissRounds,
      swissPolicy,
      qualification,
    })
  }
  return {
    initialSeedPolicy: plan.initialSeedPolicy,
    swissPairingPolicy: plan.swissPairingPolicy,
    rankingPolicy: plan.rankingPolicy,
    exactRankingTiePolicy: plan.exactRankingTiePolicy,
    stages,
  }
}

function buildSwissPolicy(
  draft: CompetitionAnnouncementStageDraft,
  matchPoints: SwissStandingPolicy['matchPoints'],
): SwissStandingPolicy {
  const numerator = u32(draft.swissPolicy.drawCredit.numerator, '瑞士和局折算分子')
  const denominator = u32(draft.swissPolicy.drawCredit.denominator, '瑞士和局折算分母')
  if (denominator === 0) invalid('瑞士和局折算分母必須大於零。')
  if (numerator > denominator) invalid('瑞士和局折算必須介於零與一之間。')
  const floor = draftPercent(draft.swissPolicy.opponentWinRateFloorPercent, '瑞士對手勝率下限')
  const cap = draftPercent(draft.swissPolicy.opponentWinRateCapPercent, '瑞士對手勝率上限')
  if (floor !== null && cap !== null && floor > cap) invalid('瑞士對手勝率下限不可高於上限。')
  if (draft.swissPolicy.excludeByesFromOpponentWinRate !== 'yes'
    && draft.swissPolicy.excludeByesFromOpponentWinRate !== 'no') invalid('請明示計算對手勝率時是否排除輪空。')
  const tieBreakers: SwissTieBreaker[] = []
  if (draft.swissPolicy.tieMode === 'configured') {
    for (const tieBreaker of draft.swissPolicy.tieBreakers) {
      if (!isTieBreaker(tieBreaker) || tieBreakers.includes(tieBreaker)) invalid('瑞士同分比較順序必須明示且不可重複。')
      tieBreakers.push(tieBreaker)
    }
    if (tieBreakers.length === 0) invalid('請明示瑞士同分比較順序，或選擇不設定。')
  } else if (draft.swissPolicy.tieMode !== 'none' || draft.swissPolicy.tieBreakers.length !== 0) {
    invalid('請明示瑞士同分比較順序，或選擇不設定。')
  }
  return {
    matchPoints,
    drawCredit: { numerator, denominator },
    opponentWinRateFloorPercent: floor,
    opponentWinRateCapPercent: cap,
    excludeByesFromOpponentWinRate: draft.swissPolicy.excludeByesFromOpponentWinRate === 'yes',
    tieBreakers,
  }
}

function parseSwissPolicy(input: unknown): SwissStandingPolicy | null {
  const value = exactRecord(input, [
    'matchPoints', 'drawCredit', 'opponentWinRateFloorPercent', 'opponentWinRateCapPercent',
    'excludeByesFromOpponentWinRate', 'tieBreakers',
  ])
  if (!value) return null
  const points = exactRecord(value.matchPoints, ['win', 'draw', 'loss', 'bye'])
  const credit = exactRecord(value.drawCredit, ['numerator', 'denominator'])
  if (!points || !['win', 'draw', 'loss', 'bye'].every(key => u32IntegerValue(points[key]))
    || !credit || !u32IntegerValue(credit.numerator) || !u32PositiveIntegerValue(credit.denominator)
    || (credit.numerator as number) > (credit.denominator as number)
    || typeof value.excludeByesFromOpponentWinRate !== 'boolean'
    || !Array.isArray(value.tieBreakers)) return null
  const floor = nullablePercent(value.opponentWinRateFloorPercent)
  const cap = nullablePercent(value.opponentWinRateCapPercent)
  if (floor === undefined || cap === undefined || (floor !== null && cap !== null && floor > cap)) return null
  const tieBreakers: SwissTieBreaker[] = []
  for (const item of value.tieBreakers) {
    if (!isTieBreaker(item) || tieBreakers.includes(item)) return null
    tieBreakers.push(item)
  }
  return {
    matchPoints: { win: points.win as number, draw: points.draw as number, loss: points.loss as number, bye: points.bye as number },
    drawCredit: { numerator: credit.numerator as number, denominator: credit.denominator as number },
    opponentWinRateFloorPercent: floor,
    opponentWinRateCapPercent: cap,
    excludeByesFromOpponentWinRate: value.excludeByesFromOpponentWinRate,
    tieBreakers,
  }
}

function draftPercent(
  draft: CompetitionAnnouncementStageDraft['swissPolicy']['opponentWinRateFloorPercent'],
  label: string,
): number | null {
  if (draft.mode === 'none') return null
  if (draft.mode !== 'value') invalid(`${label}請明示選擇是否設定。`)
  const value = u32(draft.value, label)
  if (value > 100) invalid(`${label}必須介於零到一百。`)
  return value
}

function nullablePercent(value: unknown): number | null | undefined {
  if (value === null) return null
  return u32IntegerValue(value) && value <= 100 ? value : undefined
}

function isTieBreaker(value: unknown): value is SwissTieBreaker {
  return value === 'matchPoints' || value === 'headToHeadMatchPoints' || value === 'opponentWinRate'
    || value === 'netGameWins' || value === 'seedOrder'
}

function parseQualification(
  input: unknown,
  index: number,
  earlierStages: readonly AnnouncedTournamentStage[],
): AnnouncedStageQualification | null {
  if (index === 0) {
    const first = exactRecord(input, ['type'])
    return first?.type === 'allEligibleTeams' ? { type: 'allEligibleTeams' } : null
  }
  const value = exactRecord(input, ['type', 'slots'])
  if (!value || value.type !== 'sourceSlots' || !Array.isArray(value.slots) || value.slots.length === 0) return null
  const seenSeeds = new Set<number>()
  const seenSources = new Set<string>()
  const slots: StageQualificationSlot[] = []
  for (const raw of value.slots) {
    const slot = exactRecord(raw, ['sourceStageId', 'source', 'destinationSeed'])
    if (!slot || typeof slot.sourceStageId !== 'string' || !u32PositiveIntegerValue(slot.destinationSeed)
      || seenSeeds.has(slot.destinationSeed)) return null
    const sourceStage = earlierStages.find(stage => stage.stageId === slot.sourceStageId)
    if (!sourceStage) return null
    const source = exactRecord(slot.source, slot.source && isRecord(slot.source) && slot.source.type === 'uniqueStandingRank'
      ? ['type', 'rank'] : ['type'])
    if (!source) return null
    if (source.type === 'finishedStageWinner') {
      if (sourceStage.format !== 'singleElimination' && sourceStage.format !== 'doubleElimination') return null
      const key = `${slot.sourceStageId}:winner`
      if (seenSources.has(key)) return null
      seenSources.add(key)
      slots.push({ sourceStageId: slot.sourceStageId, source: { type: 'finishedStageWinner' }, destinationSeed: slot.destinationSeed })
    } else if (source.type === 'uniqueStandingRank' && u32PositiveIntegerValue(source.rank)) {
      if (sourceStage.format !== 'roundRobin' && sourceStage.format !== 'swiss') return null
      const key = `${slot.sourceStageId}:rank:${source.rank}`
      if (seenSources.has(key)) return null
      seenSources.add(key)
      slots.push({ sourceStageId: slot.sourceStageId, source: { type: 'uniqueStandingRank', rank: source.rank }, destinationSeed: slot.destinationSeed })
    } else return null
    seenSeeds.add(slot.destinationSeed)
  }
  return { type: 'sourceSlots', slots }
}

function exactRecord(value: unknown, keys: string[]): Record<string, unknown> | null {
  if (!isRecord(value) || Reflect.ownKeys(value).length !== keys.length
    || keys.some(key => !Object.hasOwn(value, key))) return null
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && Object.getPrototypeOf(value) === Object.prototype
}

function isFormat(value: unknown): value is TournamentFormat {
  return value === 'singleElimination' || value === 'roundRobin' || value === 'doubleElimination' || value === 'swiss'
}

function positiveInteger(value: string, label: string): number {
  const result = integerText(value, label)
  if (result < 1) invalid(`${label}必須是正整數。`)
  return result
}

function nonNegativeInteger(value: string, label: string): number {
  const result = integerText(value, label)
  if (result < 0) invalid(`${label}不可小於零。`)
  return result
}

function u32(value: string, label: string): number {
  const result = nonNegativeInteger(value, label)
  if (result > MAX_U32) invalid(`${label}超出可設定範圍。`)
  return result
}

function integerText(value: string, label: string): number {
  if (!/^(0|[1-9]\d*)$/u.test(value)) invalid(`${label}必須是非負整數。`)
  const result = Number(value)
  if (!Number.isSafeInteger(result)) invalid(`${label}超出可設定範圍。`)
  return result
}

function positiveIntegerValue(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0
}

function u32PositiveIntegerValue(value: unknown): value is number {
  return positiveIntegerValue(value) && value <= MAX_U32
}

function u32IntegerValue(value: unknown): value is number {
  return nonNegativeIntegerValue(value) && value <= MAX_U32
}

function nonNegativeIntegerValue(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function normalizedId(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized && normalized === value ? normalized : null
}

function parseSourceSlots(
  draft: CompetitionAnnouncementStageDraft,
  drafts: readonly CompetitionAnnouncementStageDraft[],
  index: number,
) {
  if (draft.sourceSlots.length === 0) invalid(`第 ${index + 1} 階段請至少設定一個晉級來源名額。`)
  const seeds = new Set<number>()
  const sources = new Set<string>()
  return draft.sourceSlots.map((slot, slotIndex) => {
    const sourceStageIndex = drafts.slice(0, index).findIndex(stage => stage.stageId === slot.sourceStageId)
    if (sourceStageIndex < 0) invalid('晉級來源必須選擇之前公告的階段。')
    const sourceFormat = drafts[sourceStageIndex]?.format
    const sourceType = slot.sourceType
    if (sourceType === 'finishedStageWinner') {
      if (sourceFormat !== 'singleElimination' && sourceFormat !== 'doubleElimination') invalid('淘汰賽勝者來源只能選擇淘汰階段。')
    } else if (sourceType === 'uniqueStandingRank') {
      if (sourceFormat !== 'roundRobin' && sourceFormat !== 'swiss') invalid('唯一名次來源只能選擇循環或瑞士階段。')
    } else invalid(`第 ${index + 1} 階段第 ${slotIndex + 1} 個名額請選擇資格來源。`)
    const rank = sourceType === 'uniqueStandingRank' ? u32(slot.rank, '來源名次') : null
    const destinationSeed = u32(slot.destinationSeed, '目的種子')
    const sourceKey = `${slot.sourceStageId}:${sourceType}:${rank ?? ''}`
    if (seeds.has(destinationSeed) || sources.has(sourceKey)) invalid('目的種子與晉級來源不可重複。')
    seeds.add(destinationSeed)
    sources.add(sourceKey)
    const source = sourceType === 'finishedStageWinner'
      ? { type: 'finishedStageWinner' as const }
      : { type: 'uniqueStandingRank' as const, rank: rank! }
    return { sourceStageId: slot.sourceStageId, source, destinationSeed }
  })
}

function invalid(message: string): never { throw new Error(message) }
