import { index, integer, sqliteTable, text, uniqueIndex } from 'drizzle-orm/sqlite-core'

export const user = sqliteTable('user', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  email: text('email').notNull().unique(),
  emailVerified: integer('email_verified', { mode: 'boolean' }).notNull().default(false),
  image: text('image'),
  isAnonymous: integer('is_anonymous', { mode: 'boolean' }).notNull().default(false),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
  updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
})

export const session = sqliteTable(
  'session',
  {
    id: text('id').primaryKey(),
    expiresAt: integer('expires_at', { mode: 'timestamp' }).notNull(),
    token: text('token').notNull().unique(),
    createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
    updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
    ipAddress: text('ip_address'),
    userAgent: text('user_agent'),
    userId: text('user_id')
      .notNull()
      .references(() => user.id, { onDelete: 'cascade' }),
  },
  (table) => [index('session_user_id_idx').on(table.userId)],
)

export const account = sqliteTable(
  'account',
  {
    id: text('id').primaryKey(),
    accountId: text('account_id').notNull(),
    providerId: text('provider_id').notNull(),
    userId: text('user_id')
      .notNull()
      .references(() => user.id, { onDelete: 'cascade' }),
    accessToken: text('access_token'),
    refreshToken: text('refresh_token'),
    idToken: text('id_token'),
    accessTokenExpiresAt: integer('access_token_expires_at', { mode: 'timestamp' }),
    refreshTokenExpiresAt: integer('refresh_token_expires_at', { mode: 'timestamp' }),
    scope: text('scope'),
    password: text('password'),
    createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
    updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
  },
  (table) => [
    index('account_user_id_idx').on(table.userId),
    uniqueIndex('account_provider_account_idx').on(table.providerId, table.accountId),
  ],
)

export const verification = sqliteTable(
  'verification',
  {
    id: text('id').primaryKey(),
    identifier: text('identifier').notNull(),
    value: text('value').notNull(),
    expiresAt: integer('expires_at', { mode: 'timestamp' }).notNull(),
    createdAt: integer('created_at', { mode: 'timestamp' }),
    updatedAt: integer('updated_at', { mode: 'timestamp' }),
  },
  (table) => [index('verification_identifier_idx').on(table.identifier)],
)

export const playerProfile = sqliteTable('player_profile', {
  userId: text('user_id')
    .primaryKey()
    .references(() => user.id, { onDelete: 'cascade' }),
  displayName: text('display_name').notNull(),
  avatarUrl: text('avatar_url'),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
  updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
})

export const playerDeck = sqliteTable('player_deck', {
  userId: text('user_id')
    .primaryKey()
    .references(() => user.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  cardsJson: text('cards_json').notNull(),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
  updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
})

export const publicGameRoom = sqliteTable(
  'public_game_room',
  {
    gameId: text('game_id').primaryKey(),
    roomCode: text('room_code').notNull().unique(),
    name: text('name').notNull(),
    access: text('access').notNull(),
    status: text('status').notNull(),
    ownerUserId: text('owner_user_id').notNull(),
    playersJson: text('players_json').notNull(),
    membersJson: text('members_json').notNull(),
    enabledRuleModulesJson: text('enabled_rule_modules_json').notNull().default('[]'),
    createdAt: integer('created_at', { mode: 'timestamp' }).notNull(),
    updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull(),
  },
  (table) => [
    index('public_game_room_access_status_idx').on(table.access, table.status),
    index('public_game_room_updated_at_idx').on(table.updatedAt),
  ],
)

export const playerSavedReplay = sqliteTable(
  'player_saved_replay',
  {
    userId: text('user_id').notNull(),
    replayId: text('replay_id').notNull(),
    sourceGameId: text('source_game_id').notNull(),
    roomName: text('room_name').notNull(),
    playersJson: text('players_json').notNull(),
    resultJson: text('result_json').notNull(),
    finishedAt: text('finished_at').notNull(),
    savedAt: text('saved_at').notNull(),
  },
  (table) => [
    uniqueIndex('player_saved_replay_user_replay_idx').on(table.userId, table.replayId),
    index('player_saved_replay_user_saved_at_idx').on(table.userId, table.savedAt),
    index('player_saved_replay_replay_id_idx').on(table.replayId),
  ],
)

export const replayArchiveLifecycle = sqliteTable('replay_archive_lifecycle', {
  replayId: text('replay_id').primaryKey(),
  referenceCount: integer('reference_count').notNull(),
  version: integer('version').notNull(),
})

export const schema = {
  user,
  session,
  account,
  verification,
  playerProfile,
  playerDeck,
  publicGameRoom,
  playerSavedReplay,
  replayArchiveLifecycle,
}
