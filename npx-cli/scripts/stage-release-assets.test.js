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
    releaseAssetName('linux-x64', 'hyper-vibekanban'),
    'hyper-vibekanban-linux-x64.zip'
  );
  assert.equal(
    releaseAssetName('macos-arm64', 'hyper-vibekanban'),
    'hyper-vibekanban-macos-arm64.zip'
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
    'macos-arm64',
    'macos-x64',
  ]) {
    fs.mkdirSync(path.join(source, platform), { recursive: true });
    for (const binary of [
      'hyper-vibekanban',
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
    fs.existsSync(path.join(dest, 'hyper-vibekanban-linux-arm64.zip')),
    true
  );
  assert.equal(
    fs.existsSync(path.join(dest, 'hyper-vibekanban-macos-arm64.zip')),
    true
  );
  assert.equal(
    fs.existsSync(path.join(dest, 'hyper-vibekanban-macos-x64.zip')),
    true
  );
  assert.equal(manifest.platforms['macos-arm64']['vibe-kanban'], undefined);
  assert.equal(
    manifest.platforms['linux-x64']['hyper-vibekanban'].sha256.length,
    64
  );
  assert.equal(
    manifest.platforms['macos-arm64']['hyper-vibekanban'].sha256.length,
    64
  );
  fs.rmSync(root, { recursive: true, force: true });
});

test('cli downloads the main binary as hyper-vibekanban', () => {
  const cli = fs.readFileSync(
    path.join(__dirname, '../src/cli.ts'),
    'utf8'
  );
  assert.match(cli, /extractAndRun\(\s*"hyper-vibekanban"/);
  assert.doesNotMatch(cli, /extractAndRun\(\s*"vibe-kanban"/);
  assert.match(cli, /Downloading \$\{baseName\}/);
});
