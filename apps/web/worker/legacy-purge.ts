/**
 * Deliberately narrow D1 mutations for the issue #75 hard cutover.  Keeping
 * the statements named and data-free makes the management surface auditable:
 * room discovery data survives, while only legacy replay references disappear.
 */
export const legacyReplayDeleteStatements = [
  'DELETE FROM player_saved_replay',
  'DELETE FROM replay_archive_lifecycle',
] as const

export const resetLegacyActiveRoomStatusStatement = (
  "UPDATE public_game_room SET status = 'Waiting' WHERE status IN ('Active', 'Finished')"
)
