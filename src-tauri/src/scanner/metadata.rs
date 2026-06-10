use serde::{Deserialize, Serialize};
use tauri_plugin_shell::ShellExt;
use tokio::sync::Semaphore;
use std::time::Duration;
use tokio::time::timeout;
use std::sync::LazyLock;
use tokio::process::Command;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfprobeOutput {
    pub format: Option<FfprobeFormat>,
    pub streams: Option<Vec<FfprobeStream>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfprobeFormat {
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfprobeStream {
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MetadataResult {
    pub duration_sec: Option<f64>,
    pub resolution_text: Option<String>,
    pub codec_text: Option<String>,
}

static FFPROBE_AVAILABLE: LazyLock<bool> = LazyLock::new(|| {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
});

static FFPROBE_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(4));

pub async fn extract_metadata<P: AsRef<std::path::Path>>(
    app_handle: &tauri::AppHandle,
    path: P,
) -> AppResult<MetadataResult> {
    let path_str = path.as_ref().to_string_lossy().to_string();
    let _permit = FFPROBE_SEMAPHORE.acquire().await.map_err(|e| AppError::msg(e.to_string()))?;

    let stdout_bytes = if *FFPROBE_AVAILABLE {
        let out = Command::new("ffprobe")
            .args([
                "-v", "quiet", "-print_format", "json",
                "-show_format", "-show_streams",
                &path_str,
            ])
            .output()
            .await
            .map_err(|e| AppError::msg(format!("System ffprobe failed to execute: {}", e)))?;

        if !out.status.success() {
            return Err(AppError::msg(format!(
                "System ffprobe returned non-zero status: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        out.stdout
    } else {
        let sidecar_command = app_handle
            .shell()
            .sidecar("ffprobe")
            .map_err(|_| AppError::msg("ffprobe is not installed on your system. Please install ffmpeg to enable metadata parsing."))?
            .args([
                "-v", "quiet", "-print_format", "json",
                "-show_format", "-show_streams",
                &path_str,
            ]);

        let result = timeout(Duration::from_secs(30), sidecar_command.output()).await;
        let out = match result {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(AppError::msg(format!("ffprobe sidecar failed: {}", e))),
            Err(_) => return Err(AppError::msg("ffprobe sidecar timed out after 30 seconds")),
        };

        if !out.status.success() {
            return Err(AppError::msg(format!(
                "ffprobe sidecar returned non-zero status: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        out.stdout
    };

    let parsed: FfprobeOutput = serde_json::from_slice(&stdout_bytes)?;

    let mut result = MetadataResult {
        duration_sec: None,
        resolution_text: None,
        codec_text: None,
    };

    if let Some(format) = parsed.format {
        if let Some(duration_str) = format.duration {
            if let Ok(duration) = duration_str.parse::<f64>() {
                result.duration_sec = Some(duration);
            }
        }
    }

    if let Some(streams) = parsed.streams {
        if let Some(video_stream) = streams.iter().find(|s| s.codec_type.as_deref() == Some("video")) {
            result.codec_text = video_stream.codec_name.clone();
            if let (Some(w), Some(h)) = (video_stream.width, video_stream.height) {
                result.resolution_text = Some(format!("{}x{}", w, h));
            }
        }
    }

    Ok(result)
}

pub async fn generate_thumbnail_file(app_handle: &tauri::AppHandle, video_path: &std::path::Path, item_id: &str) -> AppResult<()> {
    let thumb_dir = crate::platform::volume::get_portable_dir().join("thumbnails");
    tokio::fs::create_dir_all(&thumb_dir).await?;

    let output_path = thumb_dir.join(format!("{}.jpg", item_id));
    let video_path_str = video_path.to_string_lossy().to_string();
    let output_path_str = output_path.to_string_lossy().to_string();

    let sidecar_command = app_handle
        .shell()
        .sidecar("ffmpeg")
        .map_err(|_| AppError::msg("ffmpeg is not installed on your system. Please install ffmpeg to enable thumbnail generation."))?
        .args([
            "-y", "-ss", "00:00:05", "-i", &video_path_str,
            "-vframes", "1", "-s", "320x180", "-f", "image2", &output_path_str,
        ]);

    let result = timeout(Duration::from_secs(30), sidecar_command.output()).await;
    let output = match result {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(AppError::msg(format!("ffmpeg sidecar failed: {}", e))),
        Err(_) => return Err(AppError::msg("ffmpeg sidecar timed out after 30 seconds")),
    };

    if !output.status.success() {
        let sidecar_command_retry = app_handle
            .shell()
            .sidecar("ffmpeg")
            .map_err(|_| AppError::msg("ffmpeg is not installed on your system."))?
            .args([
                "-y", "-ss", "00:00:00", "-i", &video_path_str,
                "-vframes", "1", "-s", "320x180", "-f", "image2", &output_path_str,
            ]);

        let result_retry = timeout(Duration::from_secs(30), sidecar_command_retry.output()).await;
        let output_retry = match result_retry {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(AppError::msg(format!("ffmpeg retry sidecar failed: {}", e))),
            Err(_) => return Err(AppError::msg("ffmpeg retry sidecar timed out after 30 seconds")),
        };

        if !output_retry.status.success() {
            return Err(AppError::msg(format!(
                "ffmpeg returned non-zero status: {}",
                String::from_utf8_lossy(&output_retry.stderr)
            )));
        }
    }

    Ok(())
}

