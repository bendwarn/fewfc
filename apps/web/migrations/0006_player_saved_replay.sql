CREATE TABLE player_saved_replay (
  user_id TEXT NOT NULL,
  replay_id TEXT NOT NULL,
  source_game_id TEXT NOT NULL,
  room_name TEXT NOT NULL,
  players_json TEXT NOT NULL,
  result_json TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  saved_at TEXT NOT NULL,
  PRIMARY KEY (user_id, replay_id)
);

CREATE INDEX player_saved_replay_user_saved_at_idx
  ON player_saved_replay (user_id, saved_at DESC);
CREATE INDEX player_saved_replay_replay_id_idx
  ON player_saved_replay (replay_id);
