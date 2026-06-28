ALTER TABLE public_game_room ADD COLUMN room_code text;

CREATE UNIQUE INDEX public_game_room_room_code_idx
ON public_game_room (room_code);
