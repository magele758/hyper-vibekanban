import https from 'https';
import type { IncomingMessage } from 'http';
import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import os from 'os';

// Public repo. Release assets are anonymous; do not point this at a private repo.
export const RELEASE_DOWNLOAD_BASE =
  'https://github.com/magele758/hyper-vibekanban/releases/download';
// Replaced during npm pack by workflow. Example: v0.1.45-20251215122030
export const BINARY_TAG = '__BINARY_TAG__';
const NPM_LATEST_URL =
  'https://registry.npmjs.org/hyper-vibekanban/latest';
const RELEASE_USER_AGENT = 'hyper-vibekanban';
const MAX_REDIRECTS = 5;
export const CACHE_DIR = path.join(os.homedir(), '.vibe-kanban', 'bin');

// Local development mode: use binaries from npx-cli/dist/ instead of GitHub releases
// Only activate if dist/ exists (i.e., running from source after local-build.sh)
export const LOCAL_DIST_DIR = path.join(__dirname, '..', 'dist');
export const LOCAL_DEV_MODE =
  fs.existsSync(LOCAL_DIST_DIR) ||
  process.env.VIBE_KANBAN_LOCAL === '1';

export interface BinaryInfo {
  sha256: string;
  size: number;
}

export interface BinaryManifest {
  latest?: string;
  platforms: Record<string, Record<string, BinaryInfo>>;
}

export interface DesktopPlatformInfo {
  file: string;
  sha256: string;
  type: string | null;
}

export interface DesktopManifest {
  platforms: Record<string, DesktopPlatformInfo>;
}

export interface DesktopBundleInfo {
  archivePath: string | null;
  dir: string;
  type: string | null;
}

type ProgressCallback = (downloaded: number, total: number) => void;

export function releaseAssetName(
  platform: string,
  binaryName: string
): string {
  return `${binaryName}-${platform}.zip`;
}

export function releaseAssetUrl(tag: string, fileName: string): string {
  return `${RELEASE_DOWNLOAD_BASE}/${encodeURIComponent(tag)}/${fileName}`;
}

function nextUrl(current: string, location: string | undefined): string {
  if (!location) {
    throw new Error(`Redirect from ${current} had no Location header`);
  }
  const next = new URL(location, current).toString();
  if (!next.startsWith('https://')) {
    throw new Error('Refusing a non-https redirect');
  }
  return next;
}

function httpsGet(
  url: string,
  redirectsLeft: number,
  onResponse: (res: IncomingMessage, finalUrl: string) => void,
  onError: (err: Error) => void
): void {
  const req = https.get(
    url,
    { headers: { 'User-Agent': RELEASE_USER_AGENT } },
    (res) => {
      const status = res.statusCode || 0;
      if (
        status === 301 ||
        status === 302 ||
        status === 303 ||
        status === 307 ||
        status === 308
      ) {
        res.resume();
        if (redirectsLeft <= 0) {
          onError(new Error(`Too many redirects fetching ${url}`));
          return;
        }
        let next: string;
        try {
          next = nextUrl(url, res.headers.location);
        } catch (err) {
          onError(err instanceof Error ? err : new Error(String(err)));
          return;
        }
        httpsGet(next, redirectsLeft - 1, onResponse, onError);
        return;
      }
      onResponse(res, url);
    }
  );
  req.on('error', (err) => onError(err));
}

function fetchJson<T>(url: string): Promise<T> {
  return new Promise((resolve, reject) => {
    httpsGet(
      url,
      MAX_REDIRECTS,
      (res, finalUrl) => {
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`HTTP ${res.statusCode} fetching ${finalUrl}`));
          return;
        }
        let data = '';
        res.on('data', (chunk: string) => (data += chunk));
        res.on('end', () => {
          try {
            resolve(JSON.parse(data) as T);
          } catch {
            reject(new Error(`Failed to parse JSON from ${finalUrl}`));
          }
        });
      },
      reject
    );
  });
}

function downloadFile(
  url: string,
  destPath: string,
  expectedSha256: string | undefined,
  onProgress?: ProgressCallback
): Promise<string> {
  const tempPath = destPath + '.tmp';
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash('sha256');

    const cleanup = () => {
      try {
        fs.unlinkSync(tempPath);
      } catch {}
    };

    httpsGet(
      url,
      MAX_REDIRECTS,
      (res, finalUrl) => {
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`HTTP ${res.statusCode} downloading ${finalUrl}`));
          return;
        }

        const file = fs.createWriteStream(tempPath);

        const totalSize = parseInt(
          res.headers['content-length'] || '0',
          10
        );
        let downloadedSize = 0;

        res.on('data', (chunk: Buffer) => {
          downloadedSize += chunk.length;
          hash.update(chunk);
          if (onProgress) onProgress(downloadedSize, totalSize);
        });
        res.pipe(file);

        file.on('finish', () => {
          file.close();
          const actualSha256 = hash.digest('hex');
          if (expectedSha256 && actualSha256 !== expectedSha256) {
            cleanup();
            reject(
              new Error(
                `Checksum mismatch: expected ${expectedSha256}, got ${actualSha256}`
              )
            );
          } else {
            try {
              fs.renameSync(tempPath, destPath);
              resolve(destPath);
            } catch (err) {
              cleanup();
              reject(err);
            }
          }
        });
        file.on('error', (err) => {
          cleanup();
          reject(err);
        });
      },
      (err) => {
        cleanup();
        reject(err);
      }
    );
  });
}

export async function ensureBinary(
  platform: string,
  binaryName: string,
  onProgress?: ProgressCallback
): Promise<string> {
  // In local dev mode, use binaries directly from npx-cli/dist/
  if (LOCAL_DEV_MODE) {
    const localZipPath = path.join(
      LOCAL_DIST_DIR,
      platform,
      `${binaryName}.zip`
    );
    if (fs.existsSync(localZipPath)) {
      return localZipPath;
    }
    throw new Error(
      `Local binary not found: ${localZipPath}\n` +
        `Run ./local-build.sh first to build the binaries.`
    );
  }

  const cacheDir = path.join(CACHE_DIR, BINARY_TAG, platform);
  const zipPath = path.join(cacheDir, `${binaryName}.zip`);

  if (fs.existsSync(zipPath)) return zipPath;

  fs.mkdirSync(cacheDir, { recursive: true });

  if (BINARY_TAG.startsWith('__')) {
    throw new Error('This package has no release tag. Rebuild it from a GitHub release.');
  }

  const manifest = await fetchJson<BinaryManifest>(
    releaseAssetUrl(BINARY_TAG, 'manifest.json')
  );
  const binaryInfo = manifest.platforms?.[platform]?.[binaryName];

  if (!binaryInfo) {
    throw new Error(
      `Binary ${binaryName} not available for ${platform}`
    );
  }

  const url = releaseAssetUrl(
    BINARY_TAG,
    releaseAssetName(platform, binaryName)
  );
  await downloadFile(url, zipPath, binaryInfo.sha256, onProgress);

  return zipPath;
}

export const DESKTOP_CACHE_DIR = path.join(
  os.homedir(),
  '.vibe-kanban',
  'desktop'
);

export async function ensureDesktopBundle(
  tauriPlatform: string,
  onProgress?: ProgressCallback
): Promise<DesktopBundleInfo> {
  // In local dev mode, use Tauri bundle from npx-cli/dist/tauri/<platform>/
  if (LOCAL_DEV_MODE) {
    const localDir = path.join(LOCAL_DIST_DIR, 'tauri', tauriPlatform);
    if (fs.existsSync(localDir)) {
      const files = fs.readdirSync(localDir);
      const archive = files.find(
        (f) => f.endsWith('.tar.gz') || f.endsWith('-setup.exe')
      );
      return {
        dir: localDir,
        archivePath: archive ? path.join(localDir, archive) : null,
        type: null,
      };
    }
    throw new Error(
      `Local desktop bundle not found: ${localDir}\n` +
        `Run './local-build.sh --desktop' first to build the Tauri app.`
    );
  }

  const cacheDir = path.join(
    DESKTOP_CACHE_DIR,
    BINARY_TAG,
    tauriPlatform
  );

  // Check if already installed (sentinel file from previous run)
  const sentinelPath = path.join(cacheDir, '.installed');
  if (fs.existsSync(sentinelPath)) {
    return { dir: cacheDir, archivePath: null, type: null };
  }

  fs.mkdirSync(cacheDir, { recursive: true });

  // Fetch the desktop manifest
  const manifest = await fetchJson<DesktopManifest>(
    releaseAssetUrl(BINARY_TAG, 'desktop-manifest.json')
  );
  const platformInfo = manifest.platforms?.[tauriPlatform];
  if (!platformInfo) {
    throw new Error(
      `Desktop app not available for platform: ${tauriPlatform}`
    );
  }

  const destPath = path.join(cacheDir, platformInfo.file);

  // Skip download if file already exists (e.g. previous failed install)
  if (!fs.existsSync(destPath)) {
    const url = releaseAssetUrl(BINARY_TAG, platformInfo.file);
    await downloadFile(url, destPath, platformInfo.sha256, onProgress);
  }

  return {
    archivePath: destPath,
    dir: cacheDir,
    type: platformInfo.type,
  };
}

export async function getLatestVersion(): Promise<string | undefined> {
  const latest = await fetchJson<{ version?: string }>(NPM_LATEST_URL);
  return latest.version;
}
