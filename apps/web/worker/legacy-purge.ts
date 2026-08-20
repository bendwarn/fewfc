/**
 * 專為 issue #75 硬切換設計的精簡 D1 變更。讓陳述式具名且不攜帶資料，可使
 * 管理介面易於稽核：房間探索資料會保留，只有舊版回放參照會消失。
 */
export const legacyReplayDeleteStatements = [
  'DELETE FROM player_saved_replay',
  'DELETE FROM replay_archive_lifecycle',
] as const

export const resetLegacyActiveRoomStatusStatement = (
  "UPDATE public_game_room SET status = 'Waiting' WHERE status IN ('Active', 'Finished')"
)
