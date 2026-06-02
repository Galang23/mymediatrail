mod models;
mod repository;
pub mod platform;
mod commands;
pub mod scanner;
pub mod media;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tauri::Manager;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Save the database in the current working directory (PWD) to make the app portable
            // Climbs out of src-tauri in dev mode to avoid tauri dev file watcher compile loops.
            let db_path = crate::platform::volume::get_portable_dir().join("mymediatrail.db");

            let db_url = format!("sqlite://{}", db_path.display());

            tauri::async_runtime::block_on(async move {
                let options = SqliteConnectOptions::from_str(&db_url)
                    .unwrap()
                    .create_if_missing(true)
                    .pragma("journal_mode", "WAL")
                    .pragma("synchronous", "NORMAL")
                    .pragma("foreign_keys", "ON")
                    .pragma("busy_timeout", "5000")
                    .pragma("cache_size", "-64000")
                    .pragma("temp_store", "MEMORY");

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(options)
                    .await
                    .unwrap();

                sqlx::migrate!("./migrations")
                    .run(&pool)
                    .await
                    .expect("Failed to run database migrations");

                app.manage(AppState { db: pool });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::run_ffprobe,
            commands::test_get_volume_uuid,
            commands::add_root,
            commands::get_roots,
            commands::trigger_scan,
            commands::get_media_items,
            commands::get_duplicate_groups,
            commands::merge_and_clean,
            commands::rebind_root,
            commands::play_media,
            commands::update_watch_status,
            commands::get_cleanup_suggestions,
            commands::delete_media_items,
            commands::get_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
