use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RootStatus {
    New,
    Active,
    Missing,
    Unresolved,
    Disabled,
}

impl From<String> for RootStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "new" => RootStatus::New,
            "active" => RootStatus::Active,
            "missing" => RootStatus::Missing,
            "unresolved" => RootStatus::Unresolved,
            "disabled" => RootStatus::Disabled,
            _ => RootStatus::New,
        }
    }
}

impl ToString for RootStatus {
    fn to_string(&self) -> String {
        match self {
            RootStatus::New => "new".to_string(),
            RootStatus::Active => "active".to_string(),
            RootStatus::Missing => "missing".to_string(),
            RootStatus::Unresolved => "unresolved".to_string(),
            RootStatus::Disabled => "disabled".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LibraryRoot {
    pub id: String,
    pub label: String,
    pub selected_path: String,
    pub normalized_selected_path: String,
    pub os_type: String,
    pub volume_uuid: Option<String>,
    pub volume_serial: Option<String>,
    pub volume_label: Option<String>,
    pub last_known_mount_path: Option<String>,
    pub root_status: String,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MediaGroup {
    pub id: String,
    pub merged_watch_status: String,
    pub preferred_instance_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MediaItem {
    pub id: String,
    pub root_id: String,
    pub relative_path: String,
    pub filename: String,
    pub extension: Option<String>,
    pub size_bytes: i64,
    pub mtime_utc: Option<String>,
    pub duration_sec: Option<f64>,
    pub resolution_text: Option<String>,
    pub codec_text: Option<String>,
    pub watch_status: String,
    pub play_count: i64,
    pub last_opened_at: Option<String>,
    pub group_id: String,
    pub metadata_status: String,
    pub metadata_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MediaFingerprint {
    pub id: String,
    pub media_item_id: String,
    pub fingerprint_hash: String,
    pub fingerprint_algo: String,
    pub fingerprint_mode: String,
    pub sample_chunk_bytes: i64,
    pub sample_count: i64,
    pub size_bytes: i64,
    pub mtime_utc: Option<String>,
    pub created_at: String,
}

impl LibraryRoot {
    pub fn get_status(&self) -> RootStatus {
        RootStatus::from(self.root_status.clone())
    }
}
