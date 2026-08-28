import type { D1Database } from '@cloudflare/workers-types'

export const SOCIAL_PROVIDERS = ['google', 'github'] as const

export type SocialProvider = typeof SOCIAL_PROVIDERS[number]

export interface SocialProviderCapabilities {
  google: boolean
  github: boolean
}

export interface SocialCredentials {
  clientId: string
  clientSecret: string
}

export interface SocialAuthEnvironment {
  GOOGLE_CLIENT_ID?: string
  GOOGLE_CLIENT_SECRET?: string
  GITHUB_CLIENT_ID?: string
  GITHUB_CLIENT_SECRET?: string
}

export interface AccountRecord {
  id: string
  providerId: string
  createdAt: Date
}

export function socialCredentials(env: SocialAuthEnvironment): Partial<Record<SocialProvider, SocialCredentials>> {
  const google = completeCredentials(env.GOOGLE_CLIENT_ID, env.GOOGLE_CLIENT_SECRET)
  const github = completeCredentials(env.GITHUB_CLIENT_ID, env.GITHUB_CLIENT_SECRET)
  return {
    ...(google ? { google } : {}),
    ...(github ? { github } : {}),
  }
}

export function socialProviderCapabilities(env: SocialAuthEnvironment): SocialProviderCapabilities {
  const credentials = socialCredentials(env)
  return {
    google: Boolean(credentials.google),
    github: Boolean(credentials.github),
  }
}

export function socialAuthConfiguration(env: SocialAuthEnvironment) {
  const credentials = socialCredentials(env)
  return {
    socialProviders: {
      ...(credentials.google ? { google: credentials.google } : {}),
      ...(credentials.github ? { github: credentials.github } : {}),
    },
    account: {
      encryptOAuthTokens: true,
      accountLinking: {
        disableImplicitLinking: true,
        allowDifferentEmails: true,
        // 名稱在 update hook 中保留，圖片則用於補 Player Profile 的空白頭像。
        updateUserInfoOnLink: true,
      },
    },
  } as const
}

function completeCredentials(clientId: string | undefined, clientSecret: string | undefined): SocialCredentials | undefined {
  if (!clientId?.trim() || !clientSecret?.trim()) return undefined
  return { clientId: clientId.trim(), clientSecret: clientSecret.trim() }
}

export function canUnlinkAccount(accounts: readonly AccountRecord[]): boolean {
  return accounts.length > 1
}

export type UnlinkAccountResult = 'unlinked' | 'last-authentication-method' | 'account-not-found'

/**
 * 用同一個 DELETE 同時驗證帳號歸屬與仍有另一個登入方式，避免兩個並行請求
 * 各自讀到兩筆帳號後一同刪除最後一筆。
 */
export async function unlinkAccountAtomically(
  database: D1Database,
  userId: string,
  accountId: string,
): Promise<UnlinkAccountResult> {
  const deleted = await database.prepare(
    `DELETE FROM account AS candidate
     WHERE candidate.id = ?
       AND candidate.user_id = ?
       AND EXISTS (
         SELECT 1 FROM account AS other
         WHERE other.user_id = candidate.user_id AND other.id <> candidate.id
       )`,
  ).bind(accountId, userId).run()
  if ((deleted.meta.changes ?? 0) === 1) return 'unlinked'

  const owned = await database.prepare(
    'SELECT 1 FROM account WHERE id = ? AND user_id = ? LIMIT 1',
  ).bind(accountId, userId).first()
  return owned ? 'last-authentication-method' : 'account-not-found'
}

export function shouldFillAvatar(existingAvatarUrl: string | null | undefined, incomingAvatarUrl: string | null | undefined): boolean {
  return !existingAvatarUrl && Boolean(incomingAvatarUrl)
}

/** Better Auth 會將 undefined 欄位略過，因此可保留既有 user.name。 */
export function providerUserUpdateWithCanonicalName<T extends { name?: unknown }>(data: T) {
  return 'name' in data ? { ...data, name: undefined } : data
}

export function canAnonymousUpgrade(hasActiveRoom: boolean): boolean {
  return !hasActiveRoom
}

export const activeAnonymousUpgradeMessage = '請先離開等待中或進行中的房間，再升級帳號。'

export function anonymousMigrationStrategy(input: {
  anonymousCreatedAt?: number
  targetCreatedAt?: number
  anonymousAvatarUrl?: string | null
  targetAvatarUrl?: string | null
}) {
  const targetIsNew = Number(input.targetCreatedAt ?? 0) >= Number(input.anonymousCreatedAt ?? Number.POSITIVE_INFINITY)
  return {
    targetIsNew,
    copyAnonymousName: targetIsNew,
    copyAnonymousDeckWhenTargetMissing: true,
    preserveTargetReplayOnConflict: true,
    copyAnonymousAvatar: targetIsNew || (!input.targetAvatarUrl && Boolean(input.anonymousAvatarUrl)),
  }
}

/**
 * 房間中的 user id 是 Durable Object 的不可變座位身分。只讀取 D1 索引來阻止
 * 升級，絕不嘗試在活躍房間內改寫座位或事件紀錄。
 */
export async function hasActiveRoom(database: D1Database, userId: string): Promise<boolean> {
  const result = await database.prepare(
    `SELECT 1
     FROM game_room_member AS member
     INNER JOIN public_game_room AS room ON room.game_id = member.game_id
     WHERE member.user_id = ? AND room.status IN ('Waiting', 'Active')
     LIMIT 1`,
  ).bind(userId).first()
  return Boolean(result)
}

interface ProfileRow {
  display_name: string
  avatar_url: string | null
}

interface UserRow {
  created_at: number
}

interface ReplayIdRow {
  replay_id: string
}

/**
 * Better Auth 會在 hook 返回後刪除匿名 user。這裡先將所有長期資料改掛到目標
 * user；房間資料刻意不在遷移範圍內。目標既有帳號以其資料為準，只有新帳號
 * 才完整承接訪客的名稱與牌組；重播以 INSERT OR IGNORE 保留目標版本。
 */
export async function migrateAnonymousData(
  database: D1Database,
  anonymousUserId: string,
  targetUserId: string,
): Promise<{ targetName?: string }> {
  if (anonymousUserId === targetUserId) return {}

  if (await hasActiveRoom(database, anonymousUserId)) {
    throw new Error(activeAnonymousUpgradeMessage)
  }

  const [anonymousUser, targetUser, anonymousProfile, targetProfile] = await Promise.all([
    database.prepare('SELECT created_at FROM user WHERE id = ?').bind(anonymousUserId).first<UserRow>(),
    database.prepare('SELECT created_at FROM user WHERE id = ?').bind(targetUserId).first<UserRow>(),
    database.prepare('SELECT display_name, avatar_url FROM player_profile WHERE user_id = ?').bind(anonymousUserId).first<ProfileRow>(),
    database.prepare('SELECT display_name, avatar_url FROM player_profile WHERE user_id = ?').bind(targetUserId).first<ProfileRow>(),
  ])

  // 新的 OAuth user 一定在訪客之後建立；既有正式帳號則保留既有 profile。
  const strategy = anonymousMigrationStrategy({
    anonymousCreatedAt: anonymousUser?.created_at,
    targetCreatedAt: targetUser?.created_at,
    anonymousAvatarUrl: anonymousProfile?.avatar_url,
    targetAvatarUrl: targetProfile?.avatar_url,
  })
  const now = Date.now()
  const replayConflicts = await database.prepare(
    `SELECT source.replay_id
     FROM player_saved_replay AS source
     INNER JOIN player_saved_replay AS target ON target.user_id = ? AND target.replay_id = source.replay_id
     WHERE source.user_id = ?`,
  ).bind(targetUserId, anonymousUserId).all<ReplayIdRow>()
  const statements = []

  if (strategy.copyAnonymousName && anonymousProfile) {
    statements.push(database.prepare('UPDATE user SET name = ?, updated_at = ? WHERE id = ?')
      .bind(anonymousProfile.display_name, now, targetUserId))
    statements.push(database.prepare(
      `UPDATE player_profile
       SET display_name = ?, avatar_url = COALESCE(?, avatar_url), updated_at = ?
       WHERE user_id = ?`,
    ).bind(anonymousProfile.display_name, anonymousProfile.avatar_url, now, targetUserId))
  } else if (strategy.copyAnonymousAvatar && anonymousProfile?.avatar_url) {
    // 既有帳號不採用訪客名稱，但可補上該帳號尚未設定的頭像。
    statements.push(database.prepare(
      'UPDATE player_profile SET avatar_url = ?, updated_at = ? WHERE user_id = ? AND avatar_url IS NULL',
    ).bind(anonymousProfile.avatar_url, now, targetUserId))
  }

  // INSERT OR IGNORE 讓既有正式帳號的牌組優先；新帳號本來沒有牌組。
  statements.push(database.prepare(
    `INSERT OR IGNORE INTO player_deck (user_id, name, cards_json, created_at, updated_at)
     SELECT ?, name, cards_json, created_at, updated_at FROM player_deck WHERE user_id = ?`,
  ).bind(targetUserId, anonymousUserId))
  // 同一 replay id 發生衝突時，目標帳號版本已存在，因此不得覆寫。
  statements.push(database.prepare(
    `INSERT OR IGNORE INTO player_saved_replay
       (user_id, replay_id, source_game_id, room_name, players_json, result_json, finished_at, saved_at)
     SELECT ?, replay_id, source_game_id, room_name, players_json, result_json, finished_at, saved_at
     FROM player_saved_replay WHERE user_id = ?`,
  ).bind(targetUserId, anonymousUserId))
  // 已存在於目標帳號的 replay 不會新增參照，但刪除訪客參照後必須同步遞減
  // 封存生命週期；其餘 replay 是同一個參照的 userId 轉移，計數維持不變。
  for (const conflict of replayConflicts.results ?? []) {
    statements.push(database.prepare(
      `UPDATE replay_archive_lifecycle
       SET reference_count = CASE WHEN reference_count > 0 THEN reference_count - 1 ELSE 0 END,
           version = version + 1
       WHERE replay_id = ?`,
    ).bind(conflict.replay_id))
  }
  statements.push(database.prepare('DELETE FROM player_saved_replay WHERE user_id = ?').bind(anonymousUserId))

  await database.batch(statements)
  return strategy.copyAnonymousName && anonymousProfile
    ? { targetName: anonymousProfile.display_name }
    : {}
}
