use serde::{Deserialize, Serialize};
use tauri_plugin_shell::ShellExt;

use tokio::sync::Semaphore;
use std::time::Duration;
use tokio::time::timeout;
use std::sync::OnceLock;
use tokio::process::Command;

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

// Global semaphore to limit concurrent ffprobe processes
static FFPROBE_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

fn get_ffprobe_semaphore() -> &'static Semaphore {
    FFPROBE_SEMAPHORE.get_or_init(|| Semaphore::new(4))
}

pub async fn extract_metadata<P: AsRef<std::path::Path>>(
    app_handle: &tauri::AppHandle,
    path: P,
) -> Result<MetadataResult, String> {
    let path_str = path.as_ref().to_string_lossy().to_string();
    
    // Acquire semaphore permit
    let _permit = get_ffprobe_semaphore().acquire().await.map_err(|e| e.to_string())?;

    // Check if system ffprobe is installed (Ref User: system ffprobe check)
    let use_system = std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    let stdout_bytes = if use_system {
        // Run standard system-installed ffprobe asynchronously
        let out = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &path_str,
            ])
            .output()
            .await
            .map_err(|e| format!("System ffprobe failed to execute: {}", e))?;

        if !out.status.success() {
            return Err(format!(
                "System ffprobe returned non-zero status: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        out.stdout
    } else {
        // Fall back to Tauri prebuilt sidecar
        let sidecar_command = app_handle
            .shell()
            .sidecar("ffprobe")
            .map_err(|_| "ffprobe is not installed on your system. Please install ffmpeg to enable metadata parsing.".to_string())?
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &path_str,
            ]);

        // 30 second timeout as specified in PRD
        let result = timeout(Duration::from_secs(30), sidecar_command.output()).await;

        let out = match result {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(format!("ffprobe sidecar failed: {}", e)),
            Err(_) => return Err("ffprobe sidecar timed out after 30 seconds".to_string()),
        };

        if !out.status.success() {
            return Err(format!(
                "ffprobe sidecar returned non-zero status: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        out.stdout
    };

    let parsed: FfprobeOutput = serde_json::from_slice(&stdout_bytes)
        .map_err(|e| format!("Failed to parse ffprobe json: {}", e))?;

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

/// Extracts a single video frame as a JPEG thumbnail using ffmpeg and saves it locally.
/// Ref User Request: system ffmpeg thumbnail generation.
pub async fn generate_thumbnail_file(video_path: &std::path::Path, item_id: &str) -> Result<(), String> {
    // 1. Check if system ffmpeg is installed
    let use_system = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !use_system {
        return Err("ffmpeg is not installed locally".to_string());
    }

    // 2. Create the thumbnails directory in the portable directory
    let thumb_dir = crate::platform::volume::get_portable_dir().join("thumbnails");
    tokio::fs::create_dir_all(&thumb_dir).await
        .map_err(|e| format!("Failed to create thumbnails dir: {}", e))?;

    let output_path = thumb_dir.join(format!("{}.jpg", item_id));

    // 3. Extract a frame at 5 seconds, resized to 320x180 (16:9 standard preview)
    let video_path_str = video_path.to_string_lossy().to_string();
    let output_path_str = output_path.to_string_lossy().to_string();

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss",
            "00:00:05",
            "-i",
            &video_path_str,
            "-vframes",
            "1",
            "-s",
            "320x180",
            "-f",
            "image2",
            &output_path_str,
        ])
        .output()
        .await
        .map_err(|e| format!("ffmpeg execution failed: {}", e))?;

    if !output.status.success() {
        // Fall back to extracting at 0 seconds if video is shorter than 5 seconds
        let output_retry = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss",
                "00:00:00",
                "-i",
                &video_path_str,
                "-vframes",
                "1",
                "-s",
                "320x180",
                "-f",
                "image2",
                &output_path_str,
            ])
            .output()
            .await
            .map_err(|e| format!("ffmpeg retry failed: {}", e))?;

        if !output_retry.status.success() {
            return Err(format!(
                "ffmpeg returned non-zero status: {}",
                String::from_utf8_lossy(&output_retry.stderr)
            ));
        }
    }

    Ok(())
}

