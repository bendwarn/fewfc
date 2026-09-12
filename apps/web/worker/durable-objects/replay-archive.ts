import { DurableObject } from 'cloudflare:workers'
import type {
  CompletedReplayDraft,
} from '../../shared/game-room'
import type { BattleRecord, PublicGameState } from '../../app/types/fewfc'
import { replaceBattleRecordPlayerLabels } from '../../shared/utils/battle-record-display'
import { resolveReplayPerspective } from '../../shared/utils/replay-perspective'
import { callRulesEngine } from '../rules-engine'

export interface ReplayFrame {
  currentStep: number
  totalSteps: number
  state: PublicGameState
  battleRecord: BattleRecord
  interaction: {
    canChooseInitialPouch: false
  }
  players: Array<{ player: string; displayName: string }>
  firstPlayer: string
}

export interface ReplayArchiveLifecycle {
  referenceCount: number
  version: number
}

interface ReplayArchiveCreateRequest {
  archive: CompletedReplayDraft
  lifecycle: ReplayArchiveLifecycle
}

/**
 * 不可變的標準封存。其介面特意限制為 create/frame/delete；標準負載永遠不會
 * 穿越瀏覽器接縫。
 */
export class ReplayArchive extends DurableObject {
  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    if (request.method === 'POST' && url.pathname.endsWith('/manage/purge-legacy')) {
      const body = await request.json() as { epoch?: unknown, sourceGameId?: unknown }
      if (typeof body.epoch !== 'string' || !body.epoch.trim()
        || typeof body.sourceGameId !== 'string' || !body.sourceGameId.trim()) {
        return Response.json({ error: 'a purge epoch and sourceGameId are required' }, { status: 400 })
      }
      return await this.purgeLegacyArchive(body.epoch.trim(), body.sourceGameId.trim())
    }
    if (request.method === 'POST' && url.pathname.endsWith('/manage/verify-legacy')) {
      const [archive, lifecycle] = await Promise.all([
        this.ctx.storage.get('archive'),
        this.ctx.storage.get('lifecycle'),
      ])
      return Response.json({ keyCount: Number(Boolean(archive)) + Number(Boolean(lifecycle)) })
    }
    if (request.method === 'POST' && url.pathname.endsWith('/manage/probe-legacy')) {
      const archive = await this.ctx.storage.get<CompletedReplayDraft>('archive')
      if (!archive) return Response.json({ status: 'absent' }, { status: 404 })
      if (typeof archive.replayId !== 'string' || typeof archive.sourceGameId !== 'string'
        || !archive.replayId.trim() || !archive.sourceGameId.trim()) {
        // A malformed archive is readable storage, but not a provable deletion
        // candidate. Preserve it and let the caller fail closed.
        return Response.json({ status: 'preserved' })
      }
      return Response.json({
        status: 'preserved',
        replayId: archive.replayId,
        sourceGameId: archive.sourceGameId,
      })
    }
    if (request.method === 'POST' && url.pathname.endsWith('/create')) {
      return await this.create(await request.json() as ReplayArchiveCreateRequest)
    }
    if (request.method === 'GET') {
      const step = Number(url.searchParams.get('step') ?? '0')
      if (!Number.isInteger(step) || step < 0) {
        return Response.json({ error: 'invalid replay step' }, { status: 400 })
      }
      return await this.frame(step, url.searchParams.get('perspective') ?? undefined)
    }
    if (request.method === 'DELETE') {
      return await this.delete(await request.json() as ReplayArchiveLifecycle)
    }
    return Response.json({ error: 'method not allowed' }, { status: 405 })
  }

  private async create(request: ReplayArchiveCreateRequest): Promise<Response> {
    const { archive, lifecycle } = request
    const existing = await this.ctx.storage.get<CompletedReplayDraft>('archive')
    if (existing && JSON.stringify(existing) !== JSON.stringify(archive)) {
      return Response.json({ error: 'replay id collision' }, { status: 409 })
    }
    const previous = await this.ctx.storage.get<ReplayArchiveLifecycle>('lifecycle')
    if (previous && lifecycle.version <= previous.version) {
      return Response.json({ replayId: archive.replayId, created: false })
    }
    if (existing) {
      await this.ctx.storage.put('lifecycle', lifecycle)
      return Response.json({ replayId: archive.replayId, created: false })
    }
    await this.ctx.storage.put('archive', archive)
    await this.ctx.storage.put('lifecycle', lifecycle)
    return Response.json({ replayId: archive.replayId, created: true }, { status: 201 })
  }

  private async delete(lifecycle: ReplayArchiveLifecycle): Promise<Response> {
    const previous = await this.ctx.storage.get<ReplayArchiveLifecycle>('lifecycle')
    if (previous && lifecycle.version <= previous.version) return new Response(null, { status: 204 })

    await this.ctx.storage.delete('archive')
    await this.ctx.storage.put('lifecycle', lifecycle)
    return new Response(null, { status: 204 })
  }

  /** 清除附加儲存空間具冪等性，會移除每個舊版封存鍵，包括已沒有存活 D1
   * 參照的值。 */
  private async purgeLegacyArchive(epoch: string, sourceGameId: string): Promise<Response> {
    const archive = await this.ctx.storage.get<CompletedReplayDraft>('archive')
    if (!archive) return Response.json({ status: 'absent', purged: false, epoch }, { status: 404 })
    // D1 的 source_game_id 只提供候選關聯；封存本身也必須確認同一房間，
    // 才能在資料保全切換中刪除，避免錯刪可讀或被錯誤索引的 Replay。
    if (archive.sourceGameId !== sourceGameId) {
      return Response.json({ status: 'preserved', purged: false, epoch })
    }
    await this.ctx.storage.deleteAll()
    return Response.json({ status: 'deleted', purged: true, epoch })
  }

  private async frame(step: number, perspective?: string): Promise<Response> {
    const archive = await this.ctx.storage.get<CompletedReplayDraft>('archive')
    if (!archive) return Response.json({ error: 'replay not found' }, { status: 404 })
    const effectivePerspective = resolveReplayPerspective(
      perspective,
      archive.firstPlayer,
      archive.players.map(player => player.player),
    )
    if (!effectivePerspective) {
      return Response.json({ error: 'invalid replay perspective' }, { status: 400 })
    }

    try {
      const frame = await callRulesEngine({
        action: { type: 'replayFrame', step, perspective: effectivePerspective },
        viewer: 'replay',
        setup: archive.setup,
        record: archive.record,
      }) as unknown as ReplayFrame
      return Response.json({
        ...frame,
        battleRecord: replaceBattleRecordPlayerLabels(frame.battleRecord, archive.players),
        players: archive.players,
        firstPlayer: archive.firstPlayer,
      } satisfies ReplayFrame)
    } catch (error) {
      const status = error instanceof Error && /ReplayStepOutOfRange/.test(error.message) ? 400 : 500
      return Response.json({ error: 'could not project replay frame' }, { status })
    }
  }
}
