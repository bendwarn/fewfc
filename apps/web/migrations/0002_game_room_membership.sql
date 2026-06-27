CREATE TABLE IF NOT EXISTS game_room_member (
  game_id text NOT NULL,
  user_id text NOT NULL,
  PRIMARY KEY (game_id, user_id)
);

CREATE INDEX IF NOT EXISTS game_room_member_user_idx
  ON game_room_member (user_id);
