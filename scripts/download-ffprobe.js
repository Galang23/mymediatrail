import fs from 'fs';
import path from 'path';
import https from 'https';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BINARIES_DIR = path.resolve(__dirname, '../src-tauri/binaries');

const TARGETS = {
  'darwin-arm64': 'ffprobe-aarch64-apple-darwin',
  'darwin-x64': 'ffprobe-x86_64-apple-darwin',
  'win32-x64': 'ffprobe-x86_64-pc-windows-msvc.exe',
  'linux-x64': 'ffprobe-x86_64-unknown-linux-gnu'
};

const URLS = {
  'darwin-arm64': 'https://unpkg.com/@ffprobe-installer/darwin-arm64@5.0.1/ffprobe',
  'darwin-x64': 'https://unpkg.com/@ffprobe-installer/darwin-x64@5.1.0/ffprobe',
  'win32-x64': 'https://unpkg.com/@ffprobe-installer/win32-x64@5.1.0/ffprobe.exe',
  'linux-x64': 'https://unpkg.com/@ffprobe-installer/linux-x64@5.2.0/ffprobe'
};

async function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    https.get(url, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        return downloadFile(res.headers.location, dest).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`Failed to download ${url}: ${res.statusCode}`));
      }

      const file = fs.createWriteStream(dest);
      res.pipe(file);

      file.on('finish', () => {
        file.close();
        fs.chmodSync(dest, 0o755);
        resolve();
      });

      file.on('error', (err) => {
        fs.unlink(dest, () => reject(err));
      });
    }).on('error', reject);
  });
}

async function main() {
  if (!fs.existsSync(BINARIES_DIR)) {
    fs.mkdirSync(BINARIES_DIR, { recursive: true });
  }

  // Get the current OS platform
  const platform = process.platform;
  
  const platformsToDownload = [];
  
  // To support universal macOS build, we download both arm64 and x64 for darwin
  if (platform === 'darwin') {
    platformsToDownload.push('darwin-arm64', 'darwin-x64');
  } else if (platform === 'win32') {
    platformsToDownload.push('win32-x64');
  } else if (platform === 'linux') {
    platformsToDownload.push('linux-x64');
  } else {
    console.error(`Unsupported platform: ${platform}`);
    process.exit(1);
  }

  for (const p of platformsToDownload) {
    const destName = TARGETS[p];
    const destPath = path.join(BINARIES_DIR, destName);
    const url = URLS[p];

    console.log(`Downloading ffprobe for ${p} to ${destPath}...`);
    try {
      await downloadFile(url, destPath);
      console.log(`Successfully downloaded ${destName}`);
    } catch (err) {
      console.error(`Failed to download ${destName}:`, err);
      process.exit(1);
    }
  }
}

main().catch(console.error);
