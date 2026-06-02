use crate::models::{LibraryRoot, MediaItem};
use sqlx::SqlitePool;
use std::path::PathBuf;

pub async fn merge_and_clean(
    db: &SqlitePool,
    preferred_item_id: &str,
    discarded_item_ids: Vec<String>,
) -> Result<(), String> {
    if discarded_item_ids.is_empty() {
        return Ok(());
    }

    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    // Fetch preferred item to get its group_id
    let preferred_item: MediaItem = sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
        .bind(preferred_item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Preferred item not found".to_string())?;

    for discarded_id in discarded_item_ids {
        if discarded_id == preferred_item_id {
            continue;
        }

        // Fetch discarded item to find its path
        let discarded_item: MediaItem = match sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
            .bind(&discarded_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
        {
            Some(item) => item,
            None => continue,
        };

        // Find the root path to resolve absolute path
        let root: LibraryRoot = sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
            .bind(&discarded_item.root_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        let abs_path = PathBuf::from(&root.selected_path).join(&discarded_item.relative_path);

        // Move to OS Trash
        if abs_path.exists() {
            if let Err(e) = trash::delete(&abs_path) {
                // Return error if trash fails to avoid inconsistent DB state
                return Err(format!("Failed to move file to trash: {} ({})", abs_path.display(), e));
            }
        }

        // Migrate play_events
        sqlx::query("UPDATE play_events SET media_item_id = ? WHERE media_item_id = ?")
            .bind(&preferred_item.id)
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        // Delete discarded item fingerprints and item itself
        sqlx::query("DELETE FROM media_fingerprints WHERE media_item_id = ?")
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM media_items WHERE id = ?")
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}
