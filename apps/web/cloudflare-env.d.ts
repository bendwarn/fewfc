/// <reference types="@cloudflare/workers-types" />

import type { CompetitionRoom } from './worker/durable-objects/competition-room'

declare global {
  interface Env {
    COMPETITION_ROOM: DurableObjectNamespace<CompetitionRoom>
  }
}
