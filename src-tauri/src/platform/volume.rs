use std::process::Command;
use std::path::{Path, PathBuf};
use sqlx::SqlitePool;
use chrono::Utc;
use crate::error::{AppError, AppResult, bail};
use crate::models::LibraryRoot;

/// Returns the portable directory. If inside `src-tauri` (development), climbs up one level to prevent file-watching infinite loops.
pub fn get_portable_dir() -> PathBuf {
    let mut pwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if pwd.ends_with("src-tauri") {
        if let Some(parent) = pwd.parent() {
            pwd = parent.to_path_buf();
        }
    }
    pwd
}

fn get_volume_uuid_linux<P: AsRef<Path>>(path: P) -> AppResult<String> {
    let path = path.as_ref();
    let output = Command::new("findmnt")
        .arg("-n")
        .arg("-o")
        .arg("UUID")
        .arg("-T")
        .arg(path)
        .output()
        .map_err(|e| AppError::msg(format!("Failed to execute findmnt: {}", e)))?;

    if !output.status.success() {
        bail!("findmnt failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() {
        bail!("findmnt returned empty UUID");
    }

    Ok(uuid)
}

pub fn get_volume_uuid<P: AsRef<Path>>(path: P) -> AppResult<String> {
    #[cfg(target_os = "linux")]
    {
        get_volume_uuid_linux(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        bail!("Unsupported operating system for volume UUID detection")
    }
}

fn get_mount_point_linux<P: AsRef<Path>>(path: P) -> AppResult<String> {
    let path = path.as_ref();
    let output = Command::new("findmnt")
        .arg("-n")
        .arg("-o")
        .arg("TARGET")
        .arg("-T")
        .arg(path)
        .output()
        .map_err(|e| AppError::msg(format!("Failed to execute findmnt: {}", e)))?;

    if !output.status.success() {
        bail!("findmnt failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let mount_point = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if mount_point.is_empty() {
        bail!("findmnt returned empty mount point");
    }

    Ok(mount_point)
}

pub fn get_mount_point<P: AsRef<Path>>(path: P) -> AppResult<String> {
    #[cfg(target_os = "linux")]
    {
        get_mount_point_linux(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            Ok(parent.to_string_lossy().to_string())
        } else {
            Ok("/".to_string())
        }
    }
}

fn list_mounted_volumes_linux() -> AppResult<Vec<(String, String)>> {
    let output = Command::new("findmnt")
        .arg("-r")
        .arg("-n")
        .arg("-o")
        .arg("TARGET,UUID")
        .output()
        .map_err(|e| AppError::msg(format!("Failed to execute findmnt: {}", e)))?;

    if !output.status.success() {
        bail!("findmnt failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut volumes = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 {
            let target = parts[0].to_string();
            let uuid = parts[1].to_string();
            if !uuid.is_empty() && uuid != "-" {
                volumes.push((target, uuid));
            }
        }
    }

    Ok(volumes)
}

pub fn list_mounted_volumes() -> AppResult<Vec<(String, String)>> {
    #[cfg(target_os = "linux")]
    {
        list_mounted_volumes_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(Vec::new())
    }
}

/// Dynamically resolves and auto-heals a library root path if mounted at a new location.
pub async fn resolve_and_heal_root(db: &SqlitePool, root_id: &str) -> AppResult<PathBuf> {
    let root: LibraryRoot = sqlx::query_as::<_, LibraryRoot>("SELECT * FROM library_roots WHERE id = ?")
        .bind(root_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::msg("Library root not found"))?;

    let selected_path = PathBuf::from(&root.selected_path);

    if selected_path.exists() {
        if root.root_status == "missing" || root.root_status == "new" {
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE library_roots SET root_status = 'active', updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(root_id)
                .execute(db)
                .await?;
        }
        return Ok(selected_path);
    }

    if let Some(ref uuid) = root.volume_uuid {
        if let Ok(mounted_volumes) = list_mounted_volumes() {
            if let Some((new_mount_path, _)) = mounted_volumes.iter().find(|(_, u)| u == uuid) {
                let last_mount = root.last_known_mount_path.as_deref().unwrap_or("");
                let old_selected = Path::new(&root.selected_path);
                let old_mount = Path::new(last_mount);

                let relative_subfolder = old_selected.strip_prefix(old_mount).unwrap_or(old_selected);
                let new_selected_path = PathBuf::from(new_mount_path).join(relative_subfolder);

                if new_selected_path.exists() {
                    let new_path_str = new_selected_path.to_string_lossy().to_string();
                    let now = Utc::now().to_rfc3339();

                    sqlx::query(
                        "UPDATE library_roots SET selected_path = ?, normalized_selected_path = ?, last_known_mount_path = ?, root_status = 'active', updated_at = ? WHERE id = ?"
                    )
                    .bind(&new_path_str)
                    .bind(new_path_str.replace('\\', "/"))
                    .bind(new_mount_path)
                    .bind(&now)
                    .bind(root_id)
                    .execute(db)
                    .await?;

                    return Ok(new_selected_path);
                }
            }
        }
    }

    if root.root_status != "missing" {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE library_roots SET root_status = 'missing', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(root_id)
            .execute(db)
            .await?;
    }

    bail!("Library root '{}' is offline (path not found)", root.label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn test_volume_uuid_and_mount_point() {
        let current_path = std::env::current_dir().unwrap();
        
        let uuid = get_volume_uuid(&current_path);
        let mount = get_mount_point(&current_path);
        
        assert!(mount.is_ok());
        let mount_path = mount.unwrap();
        assert!(!mount_path.is_empty());
    }

    #[tokio::test]
    async fn test_resolve_and_heal_root_existing() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(r#"
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
            )
        "#)
        .execute(&pool)
        .await
        .unwrap();

        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(r#"
            INSERT INTO library_roots (
                id, label, selected_path, normalized_selected_path, os_type,
                volume_uuid, root_status, created_at, updated_at
            ) VALUES ('test-root', 'Test', ?, ?, 'linux', 'mock-uuid', 'new', ?, ?)
        "#)
        .bind(&current_dir)
        .bind(current_dir.replace('\\', "/"))
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let res = resolve_and_heal_root(&pool, "test-root").await;
        assert!(res.is_ok());
        let resolved = res.unwrap();
        assert_eq!(resolved, PathBuf::from(&current_dir));

        let status: String = sqlx::query_scalar("SELECT root_status FROM library_roots WHERE id = 'test-root'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "active");
    }

    #[tokio::test]
    async fn test_resolve_and_heal_root_missing_no_uuid() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(r#"
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
            )
        "#)
        .execute(&pool)
        .await
        .unwrap();

        let non_existent = "/non/existent/path/for/mymediatrail/test";
        let now = Utc::now().to_rfc3339();

        sqlx::query(r#"
            INSERT INTO library_roots (
                id, label, selected_path, normalized_selected_path, os_type,
                volume_uuid, root_status, created_at, updated_at
            ) VALUES ('test-root-missing', 'Test', ?, ?, 'linux', NULL, 'new', ?, ?)
        "#)
        .bind(non_existent)
        .bind(non_existent.replace('\\', "/"))
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let res = resolve_and_heal_root(&pool, "test-root-missing").await;
        assert!(res.is_err());

        let status: String = sqlx::query_scalar("SELECT root_status FROM library_roots WHERE id = 'test-root-missing'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "missing");
    }
}

