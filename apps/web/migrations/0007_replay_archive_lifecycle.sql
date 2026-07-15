CREATE TABLE replay_archive_lifecycle (
  replay_id TEXT PRIMARY KEY,
  reference_count INTEGER NOT NULL,
  version INTEGER NOT NULL
);

INSERT INTO replay_archive_lifecycle (replay_id, reference_count, version)
SELECT replay_id, COUNT(*), 1
FROM player_saved_replay
GROUP BY replay_id;
