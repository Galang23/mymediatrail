pub mod fingerprint;
pub mod metadata;

use crate::models::{LibraryRoot, MediaItem};
use crate::scanner::fingerprint::generate_fingerprint;
use crate::scanner::metadata::{extract_metadata, generate_thumbnail_file};
use crate::error::{AppError, AppResult, bail};
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;
use walkdir::WalkDir;

static SCAN_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

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

pub async fn scan_root(app: AppHandle, db: SqlitePool, root: LibraryRoot) -> AppResult<()> {
    let root_mutex = {
        let mut locks = SCAN_LOCKS.lock().await;
        locks
            .entry(root.id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };

    let _lock = match root_mutex.try_lock() {
        Ok(guard) => guard,
        Err(_) => bail!("A scan is already in progress for root {}", root.label),
    };

    let root_path = match crate::platform::volume::resolve_and_heal_root(&db, &root.id).await {
        Ok(path) => path,
        Err(e) => {
            app.emit("scan:error", e.to_string()).ok();
            return Err(e);
        }
    };

    app.emit("scan:progress", ScanProgressEvent {
        files_scanned: 0,
        current_file: "Discovering directory tree structure...".to_string(),
    }).ok();

    let existing_items: Vec<MediaItem> = sqlx::query_as::<_, MediaItem>(
        "SELECT * FROM media_items WHERE root_id = ?"
    )
    .bind(&root.id)
    .fetch_all(&db)
    .await?;

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
    .await?;

    for (path, size_bytes, mtime_utc, ext) in walk_result {
        files_scanned += 1;

        let relative_path = path.strip_prefix(&root_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

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
            let fp_result = match tokio::task::spawn_blocking({
                let p = path.clone();
                move || generate_fingerprint(&p, size_bytes)
            }).await
            {
                Ok(Ok(fp)) => fp,
                _ => {
                    stats.skipped_items += 1;
                    continue;
                }
            };

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
                    is_moved = true;
                    moved_item_id = Some(old_item.id.clone());
                    existing_map.remove(&old_item.relative_path);
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
            let fp_result = match tokio::task::spawn_blocking({
                let p = path.clone();
                move || generate_fingerprint(&p, size_bytes)
            }).await
            {
                Ok(Ok(fp)) => fp,
                _ => {
                    stats.skipped_items += 1;
                    continue;
                }
            };

            let meta_result = extract_metadata(&app, &path).await;

            let (metadata_status, meta_err, duration, resolution, codec) = match meta_result {
                Ok(m) => ("ready", None, m.duration_sec, m.resolution_text, m.codec_text),
                Err(e) => ("failed", Some(e.to_string()), None, None, None),
            };

            let actual_item_id = if is_moved {
                moved_item_id.clone().unwrap()
            } else {
                Uuid::new_v4().to_string()
            };

            if metadata_status == "ready" {
                let _ = generate_thumbnail_file(&app, &path, &actual_item_id).await;
            }

            let now = Utc::now().to_rfc3339();
            let mut tx = db.begin().await?;

            if is_moved {
                let item_id = moved_item_id.clone().unwrap();
                sqlx::query(r#"
                    UPDATE media_items SET
                        relative_path = ?, filename = ?, size_bytes = ?, mtime_utc = ?,
                        duration_sec = ?, resolution_text = ?, codec_text = ?,
                        metadata_status = ?, metadata_error = ?, updated_at = ?
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
                .execute(&mut *tx).await?;

                sqlx::query("UPDATE media_fingerprints SET mtime_utc = ? WHERE media_item_id = ?")
                    .bind(&mtime_utc)
                    .bind(&item_id)
                    .execute(&mut *tx).await?;
            } else if is_new {
                let item_id = actual_item_id.clone();
                let group_id = Uuid::new_v4().to_string();

                sqlx::query("INSERT INTO media_groups (id, created_at, updated_at) VALUES (?, ?, ?)")
                    .bind(&group_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?;

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
                .execute(&mut *tx).await?;

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
                .execute(&mut *tx).await?;
            } else {
                let actual_item: MediaItem = sqlx::query_as::<_, MediaItem>(
                    "SELECT * FROM media_items WHERE root_id = ? AND relative_path = ?"
                )
                .bind(&root.id).bind(&relative_path)
                .fetch_one(&mut *tx).await?;

                sqlx::query(r#"
                    UPDATE media_items SET
                        size_bytes = ?, mtime_utc = ?, duration_sec = ?,
                        resolution_text = ?, codec_text = ?, metadata_status = ?,
                        metadata_error = ?, updated_at = ?
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
                .execute(&mut *tx).await?;

                sqlx::query(r#"
                    UPDATE media_fingerprints SET
                        fingerprint_hash = ?, fingerprint_algo = ?, fingerprint_mode = ?,
                        sample_chunk_bytes = ?, sample_count = ?, size_bytes = ?, mtime_utc = ?
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
                .execute(&mut *tx).await?;
            }

            tx.commit().await?;
        }
    }

    for (_, item) in existing_map {
        stats.missing_items += 1;
        sqlx::query("UPDATE media_items SET metadata_status = 'missing', updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(&item.id)
            .execute(&db)
            .await
            .ok();
    }

    sqlx::query("UPDATE library_roots SET last_seen_at = ?, root_status = 'active' WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(&root.id)
        .execute(&db)
        .await
        .ok();

    app.emit("scan:complete", stats).ok();

    Ok(())
}
