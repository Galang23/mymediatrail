use crate::error::{AppError, AppResult, bail};
use crate::models::{LibraryRoot, MediaItem};
use sqlx::SqlitePool;
use std::path::PathBuf;

pub async fn merge_and_clean(
    db: &SqlitePool,
    preferred_item_id: &str,
    discarded_item_ids: Vec<String>,
) -> AppResult<()> {
    if discarded_item_ids.is_empty() {
        return Ok(());
    }

    let mut tx = db.begin().await?;

    let preferred_item: MediaItem = sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
        .bind(preferred_item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::msg("Preferred item not found"))?;

    for discarded_id in discarded_item_ids {
        if discarded_id == preferred_item_id {
            continue;
        }

        let discarded_item: MediaItem = match sqlx::query_as::<_, MediaItem>("SELECT * FROM media_items WHERE id = ?")
            .bind(&discarded_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            Some(item) => item,
            None => continue,
        };

        let root: LibraryRoot = sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
            .bind(&discarded_item.root_id)
            .fetch_one(&mut *tx)
            .await?;

        let abs_path = PathBuf::from(&root.selected_path).join(&discarded_item.relative_path);

        if abs_path.exists() {
            if let Err(e) = trash::delete(&abs_path) {
                bail!("Failed to move file to trash: {} ({})", abs_path.display(), e);
            }
        }

        sqlx::query("UPDATE play_events SET media_item_id = ? WHERE media_item_id = ?")
            .bind(&preferred_item.id)
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM media_fingerprints WHERE media_item_id = ?")
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM media_items WHERE id = ?")
            .bind(&discarded_item.id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(())
}
