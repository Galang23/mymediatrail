# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-06-03

This is the initial production release of **MyMediaTrail**, transforming it from a basic prototype to a fully path-resilient, portable, and responsive desktop media catalog manager.

### Added
- **Dynamic Auto-Healing & Path Resilience**: Implemented a dynamic resolution system utilizing partition Volume UUIDs (via `findmnt` on Linux). The app automatically resolves mount relocations or drive-letter swaps and auto-heals directory paths without breaking history logs.
- **Portability Support (PWD Database)**: Changed SQLite storage location to the current working directory (PWD), making the app completely portable across folders and USB sticks.
- **Video Previews (ffmpeg frame extraction)**: Automatically generates visual thumbnail previews (`320x180` JPEGs) using system-installed `ffmpeg` at 5s/0s intervals and stores them in `./thumbnails/`.
- **Lazy-Loaded Previews**: Added the asynchronous `MediaThumbnail` React component that encodes image frames to Base64 on demand for fast UI loading.
- **Auto-Scan on Creation**: Adding a folder automatically launches a recursive scanner task in the background via `tokio::spawn` immediately.
- **System-Wide FFprobe Priority**: Prioritizes standard system-installed `ffprobe` binaries for metadata indexing, with a seamless fallback to the prebuilt sidecar.
- **Welcome Onboarding Walkthrough**: Designed a premium, glassmorphic welcome screen that guides new users through setting up locations, scanning, and tracking watch states.
- **GitHub Release CI Action**: Integrated a multi-runner GitHub Actions workflow that builds packages (.AppImage, .deb, .msi, .dmg) on tag push.
- **License**: Released the code under the permissive open-source **Apache License 2.0**.

### Changed
- **Rust Dependencies**: Integrated `base64` and platform-specific commands into `Cargo.toml`.
- **Metadata Flow**: Re-architected UUID generation to prevent scope compilation issues and speed up inserts.
- **File Exclusions**: Updated `.gitignore` to prevent database writes (`.db`, `.db-wal`, `.db-shm`) and the thumbnails folder from tracking.
- **Bumped Version**: Promoted app version from `0.1.0` to `1.0.0` globally.

### Fixed
- **Deduplication on Relocations**: Scanner now checks hashes of new files against missing ones. Moving a file within your library updates its path instead of creating duplicates, preserving watch history.
- **Pre-flight Play Checks**: Opening a missing file highlights an "offline" badge, prevents log writes, and displays a descriptive alert warning rather than failing silently.
