-- Migration: 0002_media_tables.sql
-- Create media groups, media items, fingerprints, and play events tables

CREATE TABLE media_groups (
  id TEXT PRIMARY KEY,                     -- logical_media_key (UUID)
  merged_watch_status TEXT NOT NULL DEFAULT 'unwatched',
  preferred_instance_id TEXT,              -- FK to media_items.id (best play target)
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE media_items (
  id TEXT PRIMARY KEY,
  root_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,             -- NFC-normalised
  filename TEXT NOT NULL,
  extension TEXT,
  size_bytes INTEGER NOT NULL,
  mtime_utc TEXT,
  duration_sec REAL,
  resolution_text TEXT,
  codec_text TEXT,
  watch_status TEXT NOT NULL DEFAULT 'unwatched',
  play_count INTEGER NOT NULL DEFAULT 0,
  last_opened_at TEXT,
  group_id TEXT NOT NULL,                  -- FK to media_groups.id
  metadata_status TEXT NOT NULL DEFAULT 'pending',
  metadata_error TEXT,                     -- ffprobe error message if failed
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (root_id) REFERENCES library_roots(id) ON DELETE CASCADE,
  FOREIGN KEY (group_id) REFERENCES media_groups(id) ON DELETE RESTRICT,
  UNIQUE (root_id, relative_path)
);

CREATE TABLE media_fingerprints (
  id TEXT PRIMARY KEY,
  media_item_id TEXT NOT NULL,
  fingerprint_hash TEXT NOT NULL,
  fingerprint_algo TEXT NOT NULL,
  fingerprint_mode TEXT NOT NULL,
  sample_chunk_bytes INTEGER NOT NULL,
  sample_count INTEGER NOT NULL,
  size_bytes INTEGER NOT NULL,
  mtime_utc TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE,
  UNIQUE (media_item_id)
);

CREATE TABLE play_events (
  id TEXT PRIMARY KEY,
  media_item_id TEXT NOT NULL,
  opened_at TEXT NOT NULL,
  source_action TEXT NOT NULL DEFAULT 'manual_play',
  note TEXT,
  FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
);

-- Indexes

-- Fingerprint lookup (deduplication and rebind)
CREATE INDEX idx_media_fingerprints_hash_size
  ON media_fingerprints(fingerprint_hash, size_bytes);

-- Media browsing and filtering
CREATE INDEX idx_media_items_watch_status
  ON media_items(watch_status);

CREATE INDEX idx_media_items_last_opened
  ON media_items(last_opened_at);

CREATE INDEX idx_media_items_group
  ON media_items(group_id);

CREATE INDEX idx_media_items_metadata_status
  ON media_items(metadata_status);

CREATE INDEX idx_media_items_root_status
  ON media_items(root_id, watch_status);

-- Play event history
CREATE INDEX idx_play_events_media
  ON play_events(media_item_id, opened_at DESC);
