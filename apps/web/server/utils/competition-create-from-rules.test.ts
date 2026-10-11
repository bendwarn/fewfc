import { expect, test } from 'bun:test'
import type { H3Event } from 'h3'
import { createCompetitionAnnouncementDraftFixture } from '../../tests/fixtures/competition-announcement'
import type { JsonValue } from '../../shared/competition'
import { createCompetitionFromRules } from './competition-create-from-rules'

function reverseObjectKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(reverseObjectKeys)
  if (typeof value !== 'object' || value === null) return value
  return Object.fromEntries(Object.entries(value).reverse().map(([key, child]) => [key, reverseObjectKeys(child)]))
}

test('accepts an initialized announcement whose JSON object keys are reordered', async () => {
  let initializeCalls = 0
  const event = {
    context: {
      _platform: {
        cloudflare: {
          env: {
            APP_ENV: 'development',
            COMPETITION_ROOM: {
              idFromName: (name: string) => name,
              get: () => ({
                initializeFromRules: async (request: {
                  competitionId: string
                  callerUserId: string
                  announcement: Record<string, unknown>
                }) => {
                  initializeCalls += 1
                  return {
                    ok: true,
                    value: {
                      competitionId: request.competitionId,
                      organizerUserId: request.callerUserId,
                      revision: 1,
                      projection: {
                        ok: true,
                        operation: 'registrationCreate',
                        public: {
                          announcement: {
                            ...request.announcement,
                            organizer: request.callerUserId,
                            initialHp: 137,
                            plan: reverseObjectKeys(request.announcement.plan),
                          },
                          lifecycle: 'draft',
                          revision: 0,
                          teams: [],
                        },
                      },
                    },
                  }
                },
              }),
            },
          },
        },
      },
    },
  } as unknown as H3Event

  const announcement = createCompetitionAnnouncementDraftFixture({
    enabledRuleModules: ['star'],
    teamSize: 1,
    registrationDeadline: Date.UTC(2026, 0, 1, 12, 0),
    checkInDeadline: Date.UTC(2026, 0, 1, 12, 1),
  })

  const result = await createCompetitionFromRules(event, announcement, 'organizer')

  expect(initializeCalls).toBe(1)
  const publicProjection = result.projection.public
  if (publicProjection === null || typeof publicProjection !== 'object' || Array.isArray(publicProjection)) {
    throw new Error('initialized competition must contain an object public projection')
  }
  const createdAnnouncement = publicProjection.announcement
  if (createdAnnouncement === null || typeof createdAnnouncement !== 'object' || Array.isArray(createdAnnouncement)) {
    throw new Error('initialized competition must contain an object announcement')
  }
  const expectedPlan: JsonValue = JSON.parse(JSON.stringify(announcement.plan))
  expect(createdAnnouncement.plan).toEqual(expectedPlan)
})
