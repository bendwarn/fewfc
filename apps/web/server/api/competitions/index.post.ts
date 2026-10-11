import { defineEventHandler, getQuery, readBody } from 'h3'
import { requireSession } from '../../utils/auth'
import {
  competitionCreateFromRulesError,
  createCompetitionFromRules,
} from '../../utils/competition-create-from-rules'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  if (Reflect.ownKeys(getQuery(event)).length !== 0) throw competitionCreateFromRulesError('invalidRequest')
  let body: unknown
  try {
    body = await readBody<unknown>(event)
  } catch {
    // 不反射原始 JSON 解析錯誤或尚未驗證的 body 內容。
    throw competitionCreateFromRulesError('invalidRequest')
  }
  return await createCompetitionFromRules(event, body, session.user.id)
})
