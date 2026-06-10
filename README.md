# MyMediaTrail 🎥

MyMediaTrail is a premium, portable, and path-resilient desktop media catalog manager built with **Tauri v2**, **React 19**, **TypeScript**, and **Rust**.

Designed with removable storage, USB drives, and portable drives in mind, MyMediaTrail ensures your media library cataloging remains intact even if your mount paths change.

---

## ✨ Features

* 💾 **Fully Portable**: Database (`mymediatrail.db`) and video thumbnails are kept in the current working directory, enabling you to run the app directly from a USB stick or portable hard drive.
* 🔗 **Path Resilience & Auto-Healing**: Uses partition volume UUIDs (`findmnt` on Linux) to automatically resolve drive letters or mount relocations, auto-healing media file paths without breaking your history log.
* 🎞️ **Rich Video Previews**: Generates fast thumbnail previews (`320x180` JPEGs) on-demand using system-installed `ffmpeg` / `ffprobe` and lazy-loads them in React.
* 🔍 **Smart Deduplication**: Utilizes BLAKE3 hashing to detect moved files, updating path references instead of creating duplicates and preserving watch states.
* 🚦 **Pre-Flight Play Checks**: Instantly badges files as "offline" if their containing drive is disconnected, gracefully preventing playback failures.
* 📃 **Paginated Library**: Large libraries are handled efficiently with paginated queries (`limit`/`offset`), keeping UI responsive even with thousands of items.
* ⚡ **Real-Time Scan Progress**: See filename and cumulative file count as the scanner traverses directories.
* ⌨️ **Keyboard Shortcuts**: Press `1`/`2`/`3` to switch between Library, Duplicates, and Cleanup views.
* 🔒 **Content Security Policy**: Locked-down CSP protecting against inline script injection.
* 🎨 **Premium Glassmorphic UI**: Features an onboarding screen, simple navigation, duplicate management, cleanup suggestions, and a robust status panel.

---

## 🛠️ Tech Stack

* **Frontend**: React 19, TypeScript, Vite, Vanilla CSS (Glassmorphism), Lucide React
* **Backend**: Rust, Tauri v2, thiserror (typed errors)
* **Database**: SQLite with WAL mode (managed with SQLx)
* **Processing**: Tokio (async scanner), FFmpeg & FFprobe (metadata and thumbnails), BLAKE3 (content hashing)

---

## 🚀 Getting Started

### Prerequisites

To build and run MyMediaTrail locally, you need:

1. **Rust Toolchain**: Install via [rustup](https://rustup.rs/)
2. **Node.js**: Version 18+ and **pnpm** (recommended) or npm
3. **FFmpeg & FFprobe**: Either install system-wide or the sidecar binaries will be downloaded automatically.
   * *Linux*: `sudo apt install ffmpeg` (or your distro's equivalent)
   * *macOS*: `brew install ffmpeg`
   * *Windows*: `winget install gyan.ffmpeg`

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/mymediatrail.git
   cd mymediatrail
   ```

2. Install dependencies and download sidecar binaries:
   ```bash
   pnpm install
   node scripts/download-ffprobe.js
   ```

3. Run the application in development mode:
   ```bash
   pnpm tauri dev
   ```

4. Build production binaries:
   ```bash
   pnpm tauri build
   ```

---

## 📄 License

This project is licensed under the **Apache License, Version 2.0**. See the [LICENSE](LICENSE) file for the full license text.
