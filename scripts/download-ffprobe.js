import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const BINARIES_DIR = path.resolve(__dirname, '../src-tauri/binaries');

const BINARIES_TO_DOWNLOAD = ['ffprobe', 'ffmpeg'];

const TARGETS = {
  'ffprobe': {
    'darwin-arm64': 'ffprobe-aarch64-apple-darwin',
    'darwin-x64': 'ffprobe-x86_64-apple-darwin',
    'win32-x64': 'ffprobe-x86_64-pc-windows-msvc.exe',
    'win32-ia32': 'ffprobe-i686-pc-windows-msvc.exe',
    'linux-x64': 'ffprobe-x86_64-unknown-linux-gnu',
    'linux-arm64': 'ffprobe-aarch64-unknown-linux-gnu'
  },
  'ffmpeg': {
    'darwin-arm64': 'ffmpeg-aarch64-apple-darwin',
    'darwin-x64': 'ffmpeg-x86_64-apple-darwin',
    'win32-x64': 'ffmpeg-x86_64-pc-windows-msvc.exe',
    'win32-ia32': 'ffmpeg-i686-pc-windows-msvc.exe',
    'linux-x64': 'ffmpeg-x86_64-unknown-linux-gnu',
    'linux-arm64': 'ffmpeg-aarch64-unknown-linux-gnu'
  }
};

const URLS = {
  'ffprobe': {
    'darwin-arm64': 'https://unpkg.com/@ffprobe-installer/darwin-arm64@5.0.1/ffprobe',
    'darwin-x64': 'https://unpkg.com/@ffprobe-installer/darwin-x64@5.1.0/ffprobe',
    'win32-x64': 'https://unpkg.com/@ffprobe-installer/win32-x64@5.1.0/ffprobe.exe',
    'win32-ia32': 'https://unpkg.com/@ffprobe-installer/win32-ia32@5.1.0/ffprobe.exe',
    'linux-x64': 'https://unpkg.com/@ffprobe-installer/linux-x64@5.2.0/ffprobe',
    'linux-arm64': 'https://unpkg.com/@ffprobe-installer/linux-arm64@5.2.0/ffprobe'
  },
  'ffmpeg': {
    'darwin-arm64': 'https://unpkg.com/@ffmpeg-installer/darwin-arm64@4.1.5/ffmpeg',
    'darwin-x64': 'https://unpkg.com/@ffmpeg-installer/darwin-x64@4.1.0/ffmpeg',
    'win32-x64': 'https://unpkg.com/@ffmpeg-installer/win32-x64@4.1.0/ffmpeg.exe',
    'win32-ia32': 'https://unpkg.com/@ffmpeg-installer/win32-ia32@4.1.0/ffmpeg.exe',
    'linux-x64': 'https://unpkg.com/@ffmpeg-installer/linux-x64@4.1.0/ffmpeg',
    'linux-arm64': 'https://unpkg.com/@ffmpeg-installer/linux-arm64@5.0.0/ffmpeg'
  }
};

async function downloadFile(url, dest) {
  console.log(`Downloading from ${url} to ${dest}...`);
  const response = await fetch(url);
  
  if (response.status === 301 || response.status === 302) {
    return downloadFile(response.headers.get('location'), dest);
  }
  
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
  }
  
  const buffer = await response.arrayBuffer();
  await fs.writeFile(dest, Buffer.from(buffer));
  await fs.chmod(dest, 0o755);
  console.log(`Successfully downloaded ${path.basename(dest)}`);
}

async function main() {
  await fs.mkdir(BINARIES_DIR, { recursive: true });

  const platform = process.platform;
  const platformsToDownload = [];
  
  if (platform === 'darwin') {
    platformsToDownload.push('darwin-arm64', 'darwin-x64');
  } else if (platform === 'win32') {
    platformsToDownload.push('win32-x64', 'win32-ia32');
  } else if (platform === 'linux') {
    platformsToDownload.push('linux-x64', 'linux-arm64');
  } else {
    console.error(`Unsupported platform: ${platform}`);
    process.exit(1);
  }

  for (const binary of BINARIES_TO_DOWNLOAD) {
    for (const p of platformsToDownload) {
      const destName = TARGETS[binary][p];
      const destPath = path.join(BINARIES_DIR, destName);
      const url = URLS[binary][p];

      try {
        await downloadFile(url, destPath);
      } catch (err) {
        console.error(`Failed to download ${destName}:`, err);
        process.exit(1);
      }
    }

    if (platform === 'darwin') {
      const aarch64Path = path.join(BINARIES_DIR, TARGETS[binary]['darwin-arm64']);
      const x64Path = path.join(BINARIES_DIR, TARGETS[binary]['darwin-x64']);
      const universalPath = path.join(BINARIES_DIR, `${binary}-universal-apple-darwin`);

      console.log(`Verifying ${binary} downloaded binaries architecture...`);
      try {
        const fileAarch64 = execSync(`file "${aarch64Path}"`).toString();
        const fileX64 = execSync(`file "${x64Path}"`).toString();
        console.log(`${binary} aarch64 binary: ${fileAarch64.trim()}`);
        console.log(`${binary} x64 binary: ${fileX64.trim()}`);
      } catch (e) {
        console.error(`Failed to verify ${binary} binaries with 'file' command:`, e.message);
      }

      console.log(`Creating universal binary for ${binary} at ${universalPath}...`);
      try {
        execSync(`lipo -create -output "${universalPath}" "${aarch64Path}" "${x64Path}"`);
        await fs.chmod(universalPath, 0o755);
        console.log(`Successfully created universal binary for ${binary}`);
        
        const fileUniversal = execSync(`file "${universalPath}"`).toString();
        console.log(`${binary} universal binary verification: ${fileUniversal.trim()}`);
      } catch (err) {
        console.error(`Failed to create universal binary for ${binary}:`, err.message);
        if (err.stdout) console.error(`stdout: ${err.stdout.toString()}`);
        if (err.stderr) console.error(`stderr: ${err.stderr.toString()}`);
        process.exit(1);
      }
    }
  }
}

main().catch(console.error);
