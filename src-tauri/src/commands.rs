use crate::models::{LibraryRoot, MediaItem};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn run_ffprobe(app: tauri::AppHandle) -> Result<String, String> {
    // 1. Check if standard system ffprobe is installed (Ref User: system ffprobe detection)
    let output = std::process::Command::new("ffprobe")
        .arg("-version")
        .output();
    
    match output {
        Ok(out) if out.status.success() => {
            Ok(format!("System ffprobe is installed locally:\n{}", String::from_utf8_lossy(&out.stdout)))
        }
        _ => {
            // 2. Try sidecar fallback
            use tauri_plugin_shell::ShellExt;
            let sidecar_command = app.shell().sidecar("ffprobe");
            match sidecar_command {
                Ok(sidecar) => {
                    let out = sidecar.args(["-version"]).output().await.map_err(|e| e.to_string())?;
                    Ok(format!("Sidecar ffprobe:\n{}", String::from_utf8_lossy(&out.stdout)))
                }
                Err(_) => {
                    Err("ffprobe/ffmpeg is not installed on your system. Please install ffmpeg to enable dynamic metadata indexing.".to_string())
                }
            }
        }
    }
}

#[tauri::command]
pub fn test_get_volume_uuid(path: String) -> Result<String, String> {
    crate::platform::volume::get_volume_uuid(&path)
}

#[tauri::command]
pub async fn add_root(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    label: String,
    path: String,
) -> Result<LibraryRoot, String> {
    use uuid::Uuid;
    use chrono::Utc;
    
    let now = Utc::now().to_rfc3339();
    let mount_point = crate::platform::volume::get_mount_point(&path).unwrap_or_else(|_| path.clone());
    
    let root = LibraryRoot {
        id: Uuid::new_v4().to_string(),
        label,
        selected_path: path.clone(),
        normalized_selected_path: path.replace('\\', "/"),
        os_type: std::env::consts::OS.to_string(),
        volume_uuid: crate::platform::volume::get_volume_uuid(&path).ok(),
        volume_serial: None,
        volume_label: None,
        last_known_mount_path: Some(mount_point),
        root_status: "new".to_string(),
        last_seen_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    
    let repo = crate::repository::LibraryRootRepository::new(&state.db);
    repo.insert(&root).await.map_err(|e| e.to_string())?;
    
    // Automatically trigger recursive scan in the background on add (Ref User: Auto-Scan on Add)
    let db = state.db.clone();
    let root_clone = root.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::scanner::scan_root(app, db, root_clone).await {
            eprintln!("Auto-scan on root creation failed: {}", e);
        }
    });

    Ok(root)
}

#[tauri::command]
pub async fn get_roots(state: State<'_, AppState>) -> Result<Vec<LibraryRoot>, String> {
    let repo = crate::repository::LibraryRootRepository::new(&state.db);
    repo.find_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    root_id: String,
) -> Result<(), String> {
    let root: LibraryRoot = sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
        .bind(&root_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Root not found: {}", e))?;
    
    // Spawn the scan in a background thread to prevent blocking the IPC channel!
    // Ref Issue #3 - Async Scanning.
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::scanner::scan_root(app, db, root).await {
            eprintln!("Background root scanning failed: {}", e);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn get_media_items(
    state: State<'_, AppState>,
    root_id: String,
) -> Result<Vec<MediaItem>, String> {
    sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE root_id = ?")
        .bind(&root_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub items: Vec<MediaItem>,
}

#[tauri::command]
pub async fn get_duplicate_groups(state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, String> {
    // Find all hashes that appear more than once
    let duplicate_hashes: Vec<String> = sqlx::query_scalar(
        "SELECT fingerprint_hash FROM media_fingerprints GROUP BY fingerprint_hash HAVING COUNT(media_item_id) > 1"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let mut result = Vec::new();

    for hash in duplicate_hashes {
        // Find all media items for this hash
        let items: Vec<MediaItem> = sqlx::query_as::<_, MediaItem>(
            "SELECT m.* FROM media_items m JOIN media_fingerprints f ON m.id = f.media_item_id WHERE f.fingerprint_hash = ?"
        )
        .bind(&hash)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        result.push(DuplicateGroup { hash, items });
    }

    Ok(result)
}

#[tauri::command]
pub async fn merge_and_clean(
    state: State<'_, AppState>,
    preferred_id: String,
    discarded_ids: Vec<String>,
) -> Result<(), String> {
    crate::media::merge::merge_and_clean(&state.db, &preferred_id, discarded_ids).await
}

#[tauri::command]
pub async fn rebind_root(
    state: State<'_, AppState>,
    root_id: String,
    new_path: String,
) -> Result<(), String> {
    use chrono::Utc;
    
    // Check if new path exists and verify UUID if possible
    let new_uuid = crate::platform::volume::get_volume_uuid(&new_path).ok();
    let new_mount = crate::platform::volume::get_mount_point(&new_path).unwrap_or_else(|_| new_path.clone());
    
    // Update the database
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE library_roots SET selected_path = ?, normalized_selected_path = ?, volume_uuid = COALESCE(?, volume_uuid), last_known_mount_path = ?, root_status = 'active', updated_at = ? WHERE id = ?"
    )
    .bind(&new_path)
    .bind(new_path.replace('\\', "/"))
    .bind(&new_uuid)
    .bind(&new_mount)
    .bind(&now)
    .bind(&root_id)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn play_media(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    media_item_id: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    use chrono::Utc;
    use uuid::Uuid;

    // Fetch media item first to check paths
    let item: MediaItem = sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
        .bind(&media_item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Media item not found".to_string())?;

    // 1. Resolve and auto-heal the path first
    // Ref Issue #1 & #5
    let resolved_root_path = crate::platform::volume::resolve_and_heal_root(&state.db, &item.root_id).await
        .map_err(|e| format!("Failed to resolve library path: {}", e))?;

    let abs_path = resolved_root_path.join(&item.relative_path);

    // 2. Pre-flight file existence check
    // Ref Issue #5
    if !abs_path.exists() {
        // Mark as missing in the database
        sqlx::query("UPDATE media_items SET metadata_status = 'missing', updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(&media_item_id)
            .execute(&state.db)
            .await
            .ok();
        return Err("File not found - has it been moved or is the drive disconnected?".to_string());
    }

    // Open file using default OS player
    app_handle.opener().open_path(abs_path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    let now = Utc::now().to_rfc3339();

    // Log play event and update status inside a transactional lock
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    // Log play event
    let event_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO play_events (id, media_item_id, opened_at, source_action) VALUES (?, ?, ?, 'manual_play')"
    )
    .bind(&event_id)
    .bind(&media_item_id)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Update media item watch status
    let new_status = if item.watch_status == "unwatched" { "in_progress" } else { &item.watch_status };

    sqlx::query(
        "UPDATE media_items SET play_count = play_count + 1, last_opened_at = ?, watch_status = ?, metadata_status = 'ready', updated_at = ? WHERE id = ?"
    )
    .bind(&now)
    .bind(new_status)
    .bind(&now)
    .bind(&media_item_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Propagate to group if changed
    if item.watch_status == "unwatched" {
        sqlx::query("UPDATE media_groups SET merged_watch_status = 'in_progress', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&item.group_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_watch_status(
    state: State<'_, AppState>,
    media_item_id: String,
    status: String,
) -> Result<(), String> {
    use chrono::Utc;
    let now = Utc::now().to_rfc3339();

    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    let item: MediaItem = sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
        .bind(&media_item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("UPDATE media_items SET watch_status = ?, updated_at = ? WHERE id = ?")
        .bind(&status)
        .bind(&now)
        .bind(&media_item_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // Propagate it upwards
    sqlx::query("UPDATE media_groups SET merged_watch_status = ?, updated_at = ? WHERE id = ?")
        .bind(&status)
        .bind(&now)
        .bind(&item.group_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_cleanup_suggestions(state: State<'_, AppState>) -> Result<Vec<MediaItem>, String> {
    // Exclude missing items from cleanup recommendations
    // Ref Issue #5
    sqlx::query_as::<_, MediaItem>(
        "SELECT * FROM media_items WHERE watch_status = 'watched' AND metadata_status != 'missing' ORDER BY size_bytes DESC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_media_items(
    state: State<'_, AppState>,
    media_item_ids: Vec<String>,
    permanently: bool,
) -> Result<(), String> {
    if media_item_ids.is_empty() { return Ok(()); }

    // Cache resolved root paths to avoid redundant mount system calls
    let mut resolved_roots: std::collections::HashMap<String, std::path::PathBuf> = std::collections::HashMap::new();

    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    for id in media_item_ids {
        let item: MediaItem = match sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
        {
            Some(i) => i,
            None => continue,
        };

        let root_path = if let Some(p) = resolved_roots.get(&item.root_id) {
            Some(p.clone())
        } else {
            let path = crate::platform::volume::resolve_and_heal_root(&state.db, &item.root_id).await.ok();
            if let Some(ref p) = path {
                resolved_roots.insert(item.root_id.clone(), p.clone());
            }
            path
        };

        let abs_path = if let Some(ref rp) = root_path {
            rp.join(&item.relative_path)
        } else {
            // fallback to database selected path
            let root: LibraryRoot = sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
                .bind(&item.root_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            std::path::PathBuf::from(&root.selected_path).join(&item.relative_path)
        };

        if abs_path.exists() {
            if permanently {
                std::fs::remove_file(&abs_path).map_err(|e| format!("Failed to permanently delete: {}", e))?;
            } else {
                trash::delete(&abs_path).map_err(|e| format!("Failed to move to trash: {}", e))?;
            }
        }

        // Delete associated records
        sqlx::query("DELETE FROM media_items WHERE id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_thumbnail(media_item_id: String) -> Result<Option<String>, String> {
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;
    use base64::{Engine as _, engine::general_purpose};

    let portable_dir = crate::platform::volume::get_portable_dir();
    let thumb_path = portable_dir.join("thumbnails").join(format!("{}.jpg", media_item_id));

    if !thumb_path.exists() {
        return Ok(None);
    }

    let mut file = File::open(thumb_path).await.map_err(|e| e.to_string())?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await.map_err(|e| e.to_string())?;

    let encoded = general_purpose::STANDARD.encode(buffer);
    Ok(Some(encoded))
}

