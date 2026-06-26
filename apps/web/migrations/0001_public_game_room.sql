CREATE TABLE IF NOT EXISTS "public_game_room" (
  "game_id" text PRIMARY KEY NOT NULL,
  "name" text NOT NULL,
  "access" text NOT NULL,
  "status" text NOT NULL,
  "owner_user_id" text NOT NULL,
  "players_json" text NOT NULL,
  "members_json" text NOT NULL,
  "created_at" integer NOT NULL,
  "updated_at" integer NOT NULL
);

CREATE INDEX IF NOT EXISTS "public_game_room_access_status_idx"
  ON "public_game_room" ("access", "status");

CREATE INDEX IF NOT EXISTS "public_game_room_updated_at_idx"
  ON "public_game_room" ("updated_at");
