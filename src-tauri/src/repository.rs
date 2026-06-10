use crate::error::AppResult;
use crate::models::LibraryRoot;
use sqlx::SqlitePool;

pub struct LibraryRootRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> LibraryRootRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, root: &LibraryRoot) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO library_roots (
                id, label, selected_path, normalized_selected_path, os_type,
                volume_uuid, volume_serial, volume_label, last_known_mount_path,
                root_status, last_seen_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&root.id)
        .bind(&root.label)
        .bind(&root.selected_path)
        .bind(&root.normalized_selected_path)
        .bind(&root.os_type)
        .bind(&root.volume_uuid)
        .bind(&root.volume_serial)
        .bind(&root.volume_label)
        .bind(&root.last_known_mount_path)
        .bind(&root.root_status)
        .bind(&root.last_seen_at)
        .bind(&root.created_at)
        .bind(&root.updated_at)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_all(&self) -> AppResult<Vec<LibraryRoot>> {
        Ok(sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots")
            .fetch_all(self.pool)
            .await?)
    }

    #[allow(dead_code)]
    pub async fn find_by_id(&self, id: &str) -> AppResult<Option<LibraryRoot>> {
        Ok(sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?)
    }

    #[allow(dead_code)]
    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE library_roots SET root_status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(now)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
