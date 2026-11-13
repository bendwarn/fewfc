/** 為不測試空缺原因的靜態 wire fixture 補上與其明示排名一致的 facts。 */
export function withTournamentStageFacts<T>(input: T): T {
  if (typeof input !== 'object' || input === null) return input
  const envelope = input as Record<string, any>
  const projection = envelope.projection
  if (!projection || projection.operation !== 'tournamentView'
    || Object.hasOwn(projection, 'stageFacts') || !Array.isArray(projection.public?.stages)) return input
  const stageFacts = projection.public.stages.map((stage: any) => {
    const standings = Array.isArray(stage.standings) ? stage.standings : []
    const settled = stage.status === 'finished'
      && (['singleElimination', 'doubleElimination'].includes(stage.config?.format)
        || standings.every((standing: any) => standing.rank !== null))
    return {
      stageId: stage.config?.stageId,
      settled,
      placements: standings.map((standing: any) => ({
        teamId: standing.teamId,
        placement: standing.rank === null
          ? { type: settled ? 'noPlacement' : 'unresolved' }
          : { type: 'ranked', rank: standing.rank },
      })),
      vacancies: [],
      explanations: [],
    }
  })
  return { ...envelope, projection: { ...projection, stageFacts } } as T
}

export function withFixtureExplanations<T>(input: T, stageId: string, types: Array<string | { type: string, thirdPlaceDecided: boolean }>): T {
  const result = withTournamentStageFacts(input)
  const facts = (result as any).projection.stageFacts.find((stage: any) => stage.stageId === stageId)
  if (!facts) throw new Error(`fixture 缺少階段 ${stageId}`)
  facts.explanations = types.map(type => typeof type === 'string' ? { type } : type)
  return result
}
