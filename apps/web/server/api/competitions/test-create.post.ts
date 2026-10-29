import { defineEventHandler, getQuery, readBody } from 'h3'
import { requireDevelopment } from '../../utils/app-environment'
import { requireSession } from '../../utils/auth'
import { competitionCreateFromRulesError, createTestCompetitionFromRules } from '../../utils/competition-create-from-rules'

export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  if (Reflect.ownKeys(getQuery(event)).length !== 0) throw competitionCreateFromRulesError('invalidRequest')
  let body: unknown
  try {
    body = await readBody<unknown>(event)
  } catch {
    throw competitionCreateFromRulesError('invalidRequest')
  }
  return createTestCompetitionFromRules(event, body, session.user.id)
})
