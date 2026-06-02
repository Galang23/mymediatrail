export interface LibraryRoot {
  id: string;
  label: string;
  selected_path: string;
  normalized_selected_path: string;
  volume_uuid?: string;
  last_known_mount_path: string;
  root_status: 'active' | 'missing';
  created_at: string;
  updated_at: string;
}

export interface MediaItem {
  id: string;
  group_id: string;
  root_id: string;
  relative_path: string;
  size_bytes: number;
  duration_sec?: number;
  resolution_text?: string;
  codec_text?: string;
  watch_status: 'unwatched' | 'in_progress' | 'watched';
  play_count: number;
  last_opened_at?: string;
  metadata_status: 'pending' | 'ready' | 'failed' | 'missing';
  metadata_error?: string;
  created_at: string;
  updated_at: string;
}

export interface DuplicateGroup {
  hash: string;
  items: MediaItem[];
}
