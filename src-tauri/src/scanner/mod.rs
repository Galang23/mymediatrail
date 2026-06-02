pub mod fingerprint;
pub mod metadata;

use crate::models::{LibraryRoot, MediaItem};
use crate::scanner::fingerprint::generate_fingerprint;
use crate::scanner::metadata::{extract_metadata, generate_thumbnail_file};
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;
use walkdir::WalkDir;

// Scan lock per root ID
static SCAN_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();

fn get_scan_locks() -> &'static std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>> {
    SCAN_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanProgressEvent {
    pub files_scanned: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanCompleteEvent {
    pub root_id: String,
    pub new_items: usize,
    pub changed_items: usize,
    pub missing_items: usize,
    pub skipped_items: usize,
}

pub async fn scan_root(app: AppHandle, db: SqlitePool, root: LibraryRoot) -> Result<(), String> {
    // 1. Acquire root lock
    let root_mutex = {
        let mut locks = get_scan_locks().lock().unwrap();
        locks
            .entry(root.id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    
    let _lock = match root_mutex.try_lock() {
        Ok(guard) => guard,
        Err(_) => return Err(format!("A scan is already in progress for root {}", root.label)),
    };

    // 2. Dynamically Resolve and Heal Root Path (Ref Issue #1)
    let root_path = match crate::platform::volume::resolve_and_heal_root(&db, &root.id).await {
        Ok(path) => path,
        Err(e) => {
            app.emit("scan:error", format!("Root path offline: {}", e)).ok();
            return Err(e);
        }
    };

    // Emit initial discovery progress (Ref Issue #6)
    app.emit("scan:progress", ScanProgressEvent {
        files_scanned: 0,
        current_file: "Discovering directory tree structure...".to_string(),
    }).ok();

    // 3. Fetch existing items
    let existing_items: Vec<MediaItem> = sqlx::query_as::<_, MediaItem>(
        "SELECT * FROM media_items WHERE root_id = ?"
    )
    .bind(&root.id)
    .fetch_all(&db)
    .await
    .map_err(|e| format!("Failed to fetch existing items: {}", e))?;

    let mut existing_map: HashMap<String, MediaItem> = existing_items
        .into_iter()
        .map(|item| (item.relative_path.clone(), item))
        .collect();

    let supported_exts = ["mp4", "mkv", "avi", "mov", "wmv", "m4v", "webm"];
    
    let mut files_scanned = 0;
    let mut stats = ScanCompleteEvent {
        root_id: root.id.clone(),
        new_items: 0,
        changed_items: 0,
        missing_items: 0,
        skipped_items: 0,
    };

    // 4. Walkdir traversal
    let root_path_clone = root_path.clone();
    let walk_result = tokio::task::spawn_blocking(move || {
        let mut found_files = Vec::new();
        for entry in WalkDir::new(root_path_clone).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if supported_exts.contains(&ext.to_lowercase().as_str()) {
                        if let Ok(metadata) = entry.metadata() {
                            let size = metadata.len();
                            let mtime = metadata
                                .modified()
                                .ok()
                                .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339());
                            found_files.push((entry.path().to_path_buf(), size, mtime, ext.to_lowercase()));
                        }
                    }
                }
            }
        }
        found_files
    })
    .await
    .map_err(|e| format!("Walkdir panic: {}", e))?;

    // 5. Diffing, Relocation Detection, and Processing
    for (path, size_bytes, mtime_utc, ext) in walk_result {
        files_scanned += 1;
        
        let relative_path = path.strip_prefix(&root_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/"); // normalize to forward slashes
            
        // Emit more granular progress updates (Ref Issue #6)
        if files_scanned % 10 == 0 || files_scanned < 10 {
            app.emit("scan:progress", ScanProgressEvent {
                files_scanned,
                current_file: relative_path.clone(),
            }).ok();
        }

        if size_bytes == 0 {
            stats.skipped_items += 1;
            continue;
        }

        let mut needs_processing = false;
        let mut is_new = false;
        let mut is_moved = false;
        let mut moved_item_id: Option<String> = None;

        if let Some(existing) = existing_map.remove(&relative_path) {
            if existing.size_bytes as u64 != size_bytes || existing.mtime_utc != mtime_utc {
                needs_processing = true;
                stats.changed_items += 1;
            }
        } else {
            // New path found. Could it be a moved file? (Ref Issue #2)
            // Perform pre-flight fingerprint hash to search for matching missing entries
            let fp_result = match tokio::task::spawn_blocking({
                let p = path.clone();
                move || generate_fingerprint(&p, size_bytes)
            }).await {
                Ok(Ok(fp)) => fp,
                _ => {
                    stats.skipped_items += 1;
                    continue; // Skip if hashing fails
                }
            };

            // Query if there is an existing item with the same fingerprint in this root
            let matched_missing_item: Option<MediaItem> = sqlx::query_as::<_, MediaItem>(r#"
                SELECT m.* FROM media_items m
                JOIN media_fingerprints f ON m.id = f.media_item_id
                WHERE f.fingerprint_hash = ? AND m.size_bytes = ? AND m.root_id = ?
            "#)
            .bind(&fp_result.hash)
            .bind(size_bytes as i64)
            .bind(&root.id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            if let Some(ref old_item) = matched_missing_item {
                if existing_map.contains_key(&old_item.relative_path) {
                    // Found a moved file! Heals path and retains history.
                    is_moved = true;
                    moved_item_id = Some(old_item.id.clone());
                    existing_map.remove(&old_item.relative_path); // Remove from missing list
                    needs_processing = true;
                    stats.changed_items += 1;
                }
            }

            if !is_moved {
                is_new = true;
                needs_processing = true;
                stats.new_items += 1;
            }
        }

        if needs_processing {
            // Re-hash/generate fingerprint if we didn't do it above
            let fp_result = match tokio::task::spawn_blocking({
                let p = path.clone();
                move || generate_fingerprint(&p, size_bytes)
            }).await {
                Ok(Ok(fp)) => fp,
                _ => {
                    stats.skipped_items += 1;
                    continue;
                }
            };

            // Extract technical metadata (ffprobe)
            let meta_result = extract_metadata(&app, &path).await;
            
            let (metadata_status, meta_err, duration, resolution, codec) = match meta_result {
                Ok(m) => ("ready", None, m.duration_sec, m.resolution_text, m.codec_text),
                Err(e) => ("failed", Some(e), None, None, None),
            };

            // Extract a thumbnail in the background if metadata extraction succeeded (Ref User: Thumbnail Generation)
            let actual_item_id = if is_moved {
                moved_item_id.clone().unwrap()
            } else {
                Uuid::new_v4().to_string()
            };

            if metadata_status == "ready" {
                let _ = generate_thumbnail_file(&path, &actual_item_id).await;
            }

            let now = Utc::now().to_rfc3339();
            let mut tx = db.begin().await.map_err(|e| e.to_string())?;

            if is_moved {
                // Update relocated entry (Ref Issue #2)
                let item_id = moved_item_id.clone().unwrap();
                sqlx::query(r#"
                    UPDATE media_items SET
                        relative_path = ?,
                        filename = ?,
                        size_bytes = ?,
                        mtime_utc = ?,
                        duration_sec = ?,
                        resolution_text = ?,
                        codec_text = ?,
                        metadata_status = ?,
                        metadata_error = ?,
                        updated_at = ?
                    WHERE id = ?
                "#)
                .bind(&relative_path)
                .bind(path.file_name().unwrap().to_string_lossy().to_string())
                .bind(size_bytes as i64)
                .bind(&mtime_utc)
                .bind(duration)
                .bind(&resolution)
                .bind(&codec)
                .bind(metadata_status)
                .bind(meta_err)
                .bind(&now)
                .bind(&item_id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;

                sqlx::query(r#"
                    UPDATE media_fingerprints SET
                        mtime_utc = ?
                    WHERE media_item_id = ?
                "#)
                .bind(&mtime_utc)
                .bind(&item_id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            } else if is_new {
                // Completely new file
                let item_id = actual_item_id.clone();
                let group_id = Uuid::new_v4().to_string();

                sqlx::query("INSERT INTO media_groups (id, created_at, updated_at) VALUES (?, ?, ?)")
                    .bind(&group_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await.map_err(|e| e.to_string())?;

                sqlx::query(r#"
                    INSERT INTO media_items (
                        id, root_id, relative_path, filename, extension, size_bytes, mtime_utc,
                        duration_sec, resolution_text, codec_text, group_id, metadata_status, metadata_error,
                        created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#)
                .bind(&item_id)
                .bind(&root.id)
                .bind(&relative_path)
                .bind(path.file_name().unwrap().to_string_lossy().to_string())
                .bind(&ext)
                .bind(size_bytes as i64)
                .bind(&mtime_utc)
                .bind(duration)
                .bind(&resolution)
                .bind(&codec)
                .bind(&group_id)
                .bind(metadata_status)
                .bind(meta_err)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;

                let fp_id = Uuid::new_v4().to_string();
                sqlx::query(r#"
                    INSERT INTO media_fingerprints (
                        id, media_item_id, fingerprint_hash, fingerprint_algo, fingerprint_mode,
                        sample_chunk_bytes, sample_count, size_bytes, mtime_utc, created_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#)
                .bind(&fp_id)
                .bind(&item_id)
                .bind(&fp_result.hash)
                .bind(&fp_result.algo)
                .bind(&fp_result.mode)
                .bind(fp_result.sample_chunk_bytes)
                .bind(fp_result.sample_count)
                .bind(size_bytes as i64)
                .bind(&mtime_utc)
                .bind(&now)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            } else {
                // Changed file at original relative path
                // Fetch the actual item to update it
                let actual_item: MediaItem = sqlx::query_as::<_, MediaItem>(
                    "SELECT * FROM media_items WHERE root_id = ? AND relative_path = ?"
                )
                .bind(&root.id).bind(&relative_path)
                .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;

                sqlx::query(r#"
                    UPDATE media_items SET
                        size_bytes = ?,
                        mtime_utc = ?,
                        duration_sec = ?,
                        resolution_text = ?,
                        codec_text = ?,
                        metadata_status = ?,
                        metadata_error = ?,
                        updated_at = ?
                    WHERE id = ?
                "#)
                .bind(size_bytes as i64)
                .bind(&mtime_utc)
                .bind(duration)
                .bind(&resolution)
                .bind(&codec)
                .bind(metadata_status)
                .bind(meta_err)
                .bind(&now)
                .bind(&actual_item.id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;

                sqlx::query(r#"
                    UPDATE media_fingerprints SET
                        fingerprint_hash = ?,
                        fingerprint_algo = ?,
                        fingerprint_mode = ?,
                        sample_chunk_bytes = ?,
                        sample_count = ?,
                        size_bytes = ?,
                        mtime_utc = ?
                    WHERE media_item_id = ?
                "#)
                .bind(&fp_result.hash)
                .bind(&fp_result.algo)
                .bind(&fp_result.mode)
                .bind(fp_result.sample_chunk_bytes)
                .bind(fp_result.sample_count)
                .bind(size_bytes as i64)
                .bind(&mtime_utc)
                .bind(&actual_item.id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            }

            tx.commit().await.map_err(|e| e.to_string())?;
        }
    }

    // 6. Missing Items (Ref Issue #2 & #4)
    // Mark remaining items in the DB that weren't found on disk as 'missing'
    for (_, item) in existing_map {
        stats.missing_items += 1;
        sqlx::query("UPDATE media_items SET metadata_status = 'missing', updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(&item.id)
            .execute(&db)
            .await
            .ok();
    }

    // Update root metadata status to active and last seen
    sqlx::query("UPDATE library_roots SET last_seen_at = ?, root_status = 'active' WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(&root.id)
        .execute(&db)
        .await
        .ok();

    app.emit("scan:complete", stats).ok();

    Ok(())
}
