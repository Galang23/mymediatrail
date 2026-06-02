-- Migration: 0001_init.sql
-- Connection-level PRAGMAs are not set here as they apply per-connection at runtime.
-- This migration initializes the basic root tracking model.

CREATE TABLE library_roots (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  selected_path TEXT NOT NULL,
  normalized_selected_path TEXT NOT NULL,
  os_type TEXT NOT NULL,
  volume_uuid TEXT,
  volume_serial TEXT,
  volume_label TEXT,
  last_known_mount_path TEXT,
  root_status TEXT NOT NULL DEFAULT 'new',
  last_seen_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
