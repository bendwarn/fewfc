ALTER TABLE public_game_room
ADD COLUMN enabled_rule_modules_json text NOT NULL DEFAULT '[]';
