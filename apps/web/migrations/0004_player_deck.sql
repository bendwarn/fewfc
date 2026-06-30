CREATE TABLE IF NOT EXISTS "player_deck" (
  "user_id" text PRIMARY KEY NOT NULL,
  "name" text NOT NULL,
  "cards_json" text NOT NULL,
  "created_at" integer NOT NULL,
  "updated_at" integer NOT NULL,
  FOREIGN KEY ("user_id") REFERENCES "user"("id") ON UPDATE no action ON DELETE cascade
);
