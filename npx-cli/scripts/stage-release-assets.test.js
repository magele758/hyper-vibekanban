'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const test = require('node:test');
const {
  releaseAssetName,
  stageReleaseAssets,
} = require('./stage-release-assets');

test('release asset names are flat and unique per platform', () => {
  assert.equal(
    releaseAssetName('linux-x64', 'vibe-kanban'),
    'vibe-kanban-linux-x64.zip'
  );
  assert.equal(
    releaseAssetName('windows-arm64', 'vibe-kanban-mcp'),
    'vibe-kanban-mcp-windows-arm64.zip'
  );
});

test('stages checksummed zips for every required platform', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-release-assets-'));
  const source = path.join(root, 'binaries');
  const dest = path.join(root, 'release-assets');
  for (const platform of [
    'linux-x64',
    'linux-arm64',
    'windows-x64',
    'windows-arm64',
  ]) {
    fs.mkdirSync(path.join(source, platform), { recursive: true });
    for (const binary of [
      'vibe-kanban',
      'vibe-kanban-mcp',
      'vibe-kanban-review',
    ]) {
      fs.writeFileSync(
        path.join(source, platform, `${binary}.zip`),
        `${platform}:${binary}`
      );
    }
  }
  const manifest = stageReleaseAssets(source, dest);
  assert.equal(
    fs.existsSync(path.join(dest, 'vibe-kanban-linux-arm64.zip')),
    true
  );
  assert.equal(
    manifest.platforms['linux-x64']['vibe-kanban'].sha256.length,
    64
  );
  fs.rmSync(root, { recursive: true, force: true });
});
