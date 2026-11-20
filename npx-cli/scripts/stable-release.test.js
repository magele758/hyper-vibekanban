'use strict';

const assert = require('assert');
const { execFileSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const test = require('node:test');
const {
  assertReleaseTag,
  npmVersionFromReleaseTag,
  selectPrereleaseTag,
  stableReleaseTag,
  stableVersionFromPrerelease,
  buildPublishArgs,
  planPublish,
} = require('./npm-release');
const { cutStableTarball, writeWorkspaceVersions } = require('./stable-release');

const SOURCE_TAG = 'v0.1.50-main.4.20261014164856';
const CLI = `export const BINARY_TAG = '${SOURCE_TAG}';\n`;

function writeTarball(dir, version, cliSource) {
  const root = path.join(dir, 'package');
  fs.mkdirSync(path.join(root, 'bin'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'package.json'),
    `${JSON.stringify(
      {
        name: 'hyper-vibekanban',
        version,
        private: false,
      },
      null,
      2
    )}\n`
  );
  fs.writeFileSync(path.join(root, 'bin', 'cli.js'), cliSource);
  const tgz = path.join(dir, `hyper-vibekanban-${version}.tgz`);
  execFileSync('tar', ['-czf', tgz, 'package'], { cwd: dir });
  return tgz;
}

test('stable cut drops only the prerelease suffix', () => {
  assert.equal(stableVersionFromPrerelease('0.1.50-main.4'), '0.1.50');
  assert.equal(stableReleaseTag('0.1.50'), 'v0.1.50');
  assert.throws(() => stableVersionFromPrerelease('0.1.50'), /stable/);
  assert.equal(
    npmVersionFromReleaseTag(SOURCE_TAG),
    '0.1.50-main.4'
  );
  assert.equal(
    npmVersionFromReleaseTag('v0.1.49-20261010101221'),
    '0.1.49'
  );
  assert.equal(npmVersionFromReleaseTag('remote-v0.1.34'), null);
  assert.throws(() => assertReleaseTag('remote-v0.1.34'), /release tag/);
  assert.throws(() => assertReleaseTag('relay-v0.1.2'), /release tag/);
});

test('newest npm prerelease is chosen and stable GitHub prereleases are not', () => {
  const chosen = selectPrereleaseTag([
    {
      tagName: 'v0.1.50-main.3.20261014152148',
      isPrerelease: true,
      publishedAt: '2026-10-14T15:49:55Z',
    },
    {
      tagName: 'remote-v0.1.34',
      isPrerelease: true,
      publishedAt: '2026-10-14T18:00:00Z',
    },
    {
      tagName: 'v0.1.49-20261010101221',
      isPrerelease: true,
      publishedAt: '2026-10-14T18:00:00Z',
    },
    {
      tagName: SOURCE_TAG,
      isPrerelease: true,
      publishedAt: '2026-10-14T17:22:22Z',
    },
  ]);
  assert.deepEqual(chosen, {
    tagName: SOURCE_TAG,
    version: '0.1.50-main.4',
    stableVersion: '0.1.50',
  });
  assert.throws(
    () =>
      selectPrereleaseTag([
        {
          tagName: 'v0.1.49-20261010101221',
          isPrerelease: true,
          publishedAt: '2026-10-10T11:03:56Z',
        },
      ]),
    /No npm prerelease/
  );
});

test('cut rewrites the package version and leaves the binary tag', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-cut-stable-'));
  const source = writeTarball(dir, '0.1.50-main.4', CLI);
  const dest = path.join(dir, 'out');
  const plan = cutStableTarball(source, dest);
  assert.equal(plan.version, '0.1.50');
  assert.equal(plan.tag, 'v0.1.50');
  assert.equal(path.isAbsolute(plan.tarball), true);
  const packedJson = execFileSync(
    'tar',
    ['-xOf', plan.tarball, 'package/package.json'],
    { encoding: 'utf8' }
  );
  const packedCli = execFileSync(
    'tar',
    ['-xOf', plan.tarball, 'package/bin/cli.js'],
    { encoding: 'utf8' }
  );
  assert.equal(JSON.parse(packedJson).version, '0.1.50');
  assert.equal(packedCli, CLI);
  const publishPlan = planPublish({
    name: 'hyper-vibekanban',
    version: JSON.parse(packedJson).version,
    cliSource: packedCli,
  });
  assert.equal(publishPlan.tag, 'latest');
  const relative = path.relative(dir, plan.tarball);
  assert.equal(path.isAbsolute(buildPublishArgs(relative, 'latest')[1]), true);
  fs.rmSync(dir, { recursive: true, force: true });
});

test('workspace version files keep their formatting', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-versions-'));
  const pkg = '{\n  "name": "vibe-kanban",\n  "version": "0.1.50-main.4"\n}\n';
  const lock = [
    '{',
    '  "name": "hyper-vibekanban",',
    '  "version": "0.1.50-main.4",',
    '  "packages": {',
    '    "": {',
    '      "version": "0.1.50-main.4"',
    '    }',
    '  }',
    '}',
    '',
  ].join('\n');
  fs.mkdirSync(path.join(root, 'npx-cli'), { recursive: true });
  fs.mkdirSync(path.join(root, 'packages', 'local-web'), { recursive: true });
  fs.writeFileSync(path.join(root, 'package.json'), pkg);
  fs.writeFileSync(path.join(root, 'npx-cli', 'package.json'), pkg);
  fs.writeFileSync(path.join(root, 'npx-cli', 'package-lock.json'), lock);
  fs.writeFileSync(path.join(root, 'packages', 'local-web', 'package.json'), pkg);
  const files = writeWorkspaceVersions('0.1.50', root);
  assert.deepEqual(files.sort(), [
    'npx-cli/package-lock.json',
    'npx-cli/package.json',
    'package.json',
    'packages/local-web/package.json',
  ]);
  const updated = fs.readFileSync(path.join(root, 'package.json'), 'utf8');
  assert.equal(
    updated,
    '{\n  "name": "vibe-kanban",\n  "version": "0.1.50"\n}\n'
  );
  const updatedLock = fs.readFileSync(
    path.join(root, 'npx-cli', 'package-lock.json'),
    'utf8'
  );
  assert.equal(updatedLock.includes('0.1.50-main.4'), false);
  assert.equal((updatedLock.match(/"version": "0\.1\.50"/g) || []).length, 2);
  assert.deepEqual(writeWorkspaceVersions('0.1.50', root), []);
  assert.throws(() => writeWorkspaceVersions('0.1.51-main.0', root), /stable/);
  fs.rmSync(root, { recursive: true, force: true });
});

test('stable-release CLI prints a shell-safe plan and rejects latest', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-stable-cli-'));
  const source = writeTarball(dir, '0.1.50-main.4', CLI);
  const script = path.join(__dirname, 'stable-release.js');
  const cut = spawnSync(process.execPath, [script, 'cut', source, dir], {
    encoding: 'utf8',
  });
  assert.equal(cut.status, 0, cut.stderr);
  assert.match(cut.stdout, /^source_version=0\.1\.50-main\.4\n/);
  assert.match(cut.stdout, /stable_version=0\.1\.50\n/);
  assert.match(cut.stdout, /stable_tag=v0\.1\.50\n/);
  assert.match(cut.stdout, /tarball=\//);

  const stable = writeTarball(dir, '0.1.50', CLI);
  const refused = spawnSync(
    process.execPath,
    [script, 'assert-prerelease', stable],
    { encoding: 'utf8' }
  );
  assert.equal(refused.status, 1);
  assert.match(refused.stderr, /latest/);
  const allowed = spawnSync(
    process.execPath,
    [script, 'assert-prerelease', source],
    { encoding: 'utf8' }
  );
  assert.equal(allowed.status, 0, allowed.stderr);
  assert.match(allowed.stdout, /dist-tag main/);
  fs.rmSync(dir, { recursive: true, force: true });
});
