CREATE TABLE replay_archive_lifecycle (
  replay_id TEXT PRIMARY KEY,
  reference_count INTEGER NOT NULL,
  version INTEGER NOT NULL
);
