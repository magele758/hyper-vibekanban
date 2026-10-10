#!/usr/bin/env node
'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const PLATFORMS = [
  'linux-x64',
  'linux-arm64',
  'windows-x64',
  'windows-arm64',
  'macos-arm64',
];
// Main zip/manifest key matches npx-cli extractAndRun("hyper-vibekanban").
const BINARIES = [
  'hyper-vibekanban',
  'vibe-kanban-mcp',
  'vibe-kanban-review',
];

function releaseAssetName(platform, binaryName) {
  return `${binaryName}-${platform}.zip`;
}

function stageReleaseAssets(sourceDir, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  const manifest = { platforms: {} };
  const missing = [];
  for (const platform of PLATFORMS) {
    manifest.platforms[platform] = {};
    for (const binary of BINARIES) {
      const from = path.join(sourceDir, platform, `${binary}.zip`);
      const fileName = releaseAssetName(platform, binary);
      if (!fs.existsSync(from)) {
        missing.push(from);
        continue;
      }
      const data = fs.readFileSync(from);
      fs.writeFileSync(path.join(destDir, fileName), data);
      manifest.platforms[platform][binary] = {
        sha256: crypto.createHash('sha256').update(data).digest('hex'),
        size: data.length,
      };
    }
  }
  if (missing.length > 0) {
    throw new Error(`Missing platform zip:\n${missing.join('\n')}`);
  }
  fs.writeFileSync(
    path.join(destDir, 'manifest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`
  );
  return manifest;
}

function main() {
  const sourceDir = process.argv[2];
  const destDir = process.argv[3];
  if (!sourceDir || !destDir) {
    console.error(
      'Usage: node scripts/stage-release-assets.js <binaries-dir> <out-dir>'
    );
    process.exit(1);
  }
  try {
    stageReleaseAssets(sourceDir, destDir);
    console.log(`Staged release assets in ${destDir}`);
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
}

module.exports = {
  BINARIES,
  PLATFORMS,
  releaseAssetName,
  stageReleaseAssets,
};

if (require.main === module) {
  main();
}
