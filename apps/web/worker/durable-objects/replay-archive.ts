import { DurableObject } from 'cloudflare:workers'
import type {
  CompletedReplayDraft,
} from '../../shared/game-room'
import type { PublicGameEvent, PublicGameState } from '../../app/types/fewfc'
import { callRulesEngine } from '../rules-engine'

export interface ReplayFrame {
  currentStep: number
  totalSteps: number
  state: PublicGameState
  events: PublicGameEvent[]
  interaction: {
    canPass: false
    hasOptionalEffect: false
    canRetrieveDiscard: false
    discardRetrievalAction: null
    canChooseInitialPouch: false
    canTriggerPouch: false
    pouchChainAction: null
    secretStrategyActions: []
  }
}

/**
 * Immutable canonical archive. Its interface is deliberately limited to
 * create/frame/delete; the canonical payload never crosses the browser seam.
 */
export class ReplayArchive extends DurableObject {
  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    if (request.method === 'POST' && url.pathname.endsWith('/create')) {
      return await this.create(await request.json() as CompletedReplayDraft)
    }
    if (request.method === 'GET') {
      const step = Number(url.searchParams.get('step') ?? '0')
      if (!Number.isInteger(step) || step < 0) {
        return Response.json({ error: 'invalid replay step' }, { status: 400 })
      }
      return await this.frame(step)
    }
    if (request.method === 'DELETE') {
      await this.ctx.storage.deleteAll()
      return new Response(null, { status: 204 })
    }
    return Response.json({ error: 'method not allowed' }, { status: 405 })
  }

  private async create(archive: CompletedReplayDraft): Promise<Response> {
    const existing = await this.ctx.storage.get<CompletedReplayDraft>('archive')
    if (existing) {
      if (JSON.stringify(existing) !== JSON.stringify(archive)) {
        return Response.json({ error: 'replay id collision' }, { status: 409 })
      }
      return Response.json({ replayId: archive.replayId, created: false })
    }
    await this.ctx.storage.put('archive', archive)
    return Response.json({ replayId: archive.replayId, created: true }, { status: 201 })
  }

  private async frame(step: number): Promise<Response> {
    const archive = await this.ctx.storage.get<CompletedReplayDraft>('archive')
    if (!archive) return Response.json({ error: 'replay not found' }, { status: 404 })

    try {
      const frame = await callRulesEngine({
        action: { type: 'replayFrame', step },
        viewer: 'replay',
        setup: archive.setup,
        record: archive.record,
      }) as unknown as ReplayFrame
      return Response.json(frame)
    } catch (error) {
      const status = error instanceof Error && /ReplayStepOutOfRange/.test(error.message) ? 400 : 500
      return Response.json({ error: 'could not project replay frame' }, { status })
    }
  }
}
