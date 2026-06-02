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
* 🎨 **Premium Glassmorphic UI**: Features an onboarding screen, simple navigation, search filter, duplicate management, and a robust status panel.

---

## 🛠️ Tech Stack

* **Frontend**: React 19, TypeScript, Vite, Vanilla CSS (Glassmorphism), Lucide React
* **Backend**: Rust, Tauri v2
* **Database**: SQLite (managed with SQLx)
* **Processing**: Tokio (asynchronous scanner tasks), FFmpeg & FFprobe (metadata and thumbnail extraction)

---

## 🚀 Getting Started

### Prerequisites

To build and run MyMediaTrail locally, you need:

1. **Rust Toolchain**: Install via [rustup](https://rustup.rs/)
2. **Node.js**: Version 18+ (with `npm` or `pnpm`)
3. **FFmpeg & FFprobe**: Ensure they are installed and available in your system's PATH.
   * *Linux*: `sudo apt install ffmpeg` (or your distro's equivalent)
   * *macOS*: `brew install ffmpeg`
   * *Windows*: Install via Winget `winget install gyan.ffmpeg` or download from the official page.

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/mymediatrail.git
   cd mymediatrail
   ```

2. Install dependencies:
   ```bash
   npm install
   # or
   pnpm install
   ```

3. Run the application in development mode:
   ```bash
   npm run tauri dev
   # or
   pnpm tauri dev
   ```

4. Build production binaries:
   ```bash
   npm run tauri build
   # or
   pnpm tauri build
   ```

---

## 📄 License

This project is licensed under the **Apache License, Version 2.0**. See the [LICENSE](LICENSE) file for the full license text.
