'use strict';

const assert = require('assert');
const { execFileSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const test = require('node:test');
const {
  injectReleaseConfig,
  npmSupportsOidc,
  distTagForVersion,
  planPublish,
  npmViewIsMissing,
  stripEmptyNpmAuth,
  buildPublishArgs,
} = require('./npm-release');

const SOURCE = [
  "export const R2_BASE_URL = '__R2_PUBLIC_URL__';",
  "export const BINARY_TAG = '__BINARY_TAG__';",
  '',
].join('\n');

const PUBLIC_URL = 'https://binaries.example.test/vk';
const TAG = 'v0.1.45-20261009120000';

test('injects the release URL and tag without keeping placeholders', () => {
  const next = injectReleaseConfig(SOURCE, {
    publicUrl: PUBLIC_URL,
    binaryTag: TAG,
  });
  assert.equal(next.includes('__R2_PUBLIC_URL__'), false);
  assert.equal(next.includes('__BINARY_TAG__'), false);
  assert.equal(next.includes(PUBLIC_URL), true);
  assert.equal(next.includes(TAG), true);
});

test('rejects an unsafe URL without echoing it', () => {
  const bad = "https://binaries.example.test/a'b";
  assert.throws(
    () =>
      injectReleaseConfig(SOURCE, {
        publicUrl: bad,
        binaryTag: TAG,
      }),
    (err) => {
      assert.equal(err.message.includes(bad), false);
      assert.match(err.message, /R2_BINARIES_PUBLIC_URL/);
      return true;
    }
  );
});

test('rejects an empty public URL', () => {
  assert.throws(
    () => injectReleaseConfig(SOURCE, { publicUrl: '', binaryTag: TAG }),
    /R2_BINARIES_PUBLIC_URL/
  );
});

test('dist-tag is latest only for stable versions', () => {
  assert.equal(distTagForVersion('0.1.45'), 'latest');
  assert.equal(distTagForVersion('0.1.44-3e90.0'), '3e90');
  assert.throws(() => distTagForVersion('0.1.44-latest.1'), /dist-tag/);
});

test('planPublish refuses placeholders and the wrong package name', () => {
  const plan = planPublish({
    name: 'hyper-vibekanban',
    version: '0.1.45',
    cliSource: 'const url = "https://binaries.example.test";',
  });
  assert.deepEqual(plan, {
    name: 'hyper-vibekanban',
    version: '0.1.45',
    tag: 'latest',
  });
  assert.throws(
    () =>
      planPublish({
        name: 'hyper-vibekanban',
        version: '0.1.45',
        cliSource: SOURCE,
      }),
    /placeholders/
  );
  assert.throws(
    () =>
      planPublish({
        name: 'other',
        version: '0.1.45',
        cliSource: 'ok',
      }),
    /hyper-vibekanban/
  );
});

test('npm OIDC support and registry 404 detection', () => {
  assert.equal(npmSupportsOidc('11.5.1'), true);
  assert.equal(npmSupportsOidc('11.12.0'), true);
  assert.equal(npmSupportsOidc('11.5.0'), false);
  assert.equal(npmSupportsOidc('10.9.7'), false);
  assert.equal(
    npmViewIsMissing(1, 'npm error code E404\nNo match found for version'),
    true
  );
  assert.equal(npmViewIsMissing(0, '0.1.45'), false);
  assert.equal(npmViewIsMissing(1, 'npm error code ECONNRESET'), false);
});

test('strips only empty auth lines from npmrc text', () => {
  const next = stripEmptyNpmAuth(
    [
      'registry=https://registry.npmjs.org/',
      '//registry.npmjs.org/:_authToken=${NODE_AUTH_TOKEN}',
      '//registry.npmjs.org/:_authToken=',
      'fund=false',
    ].join('\n')
  );
  assert.equal(next.includes('_authToken'), false);
  assert.equal(next.includes('fund=false'), true);
});

test('publish args request provenance, public access, and the dist-tag', () => {
  assert.deepEqual(buildPublishArgs('pkg.tgz', 'latest'), [
    'publish',
    'pkg.tgz',
    '--provenance',
    '--access',
    'public',
    '--tag',
    'latest',
  ]);
});

function writeTarball(dir, version, cliSource) {
  const root = path.join(dir, 'package');
  fs.mkdirSync(path.join(root, 'bin'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'package.json'),
    JSON.stringify({
      name: 'hyper-vibekanban',
      version,
      private: false,
    })
  );
  fs.writeFileSync(path.join(root, 'bin', 'cli.js'), cliSource);
  const tgz = path.join(dir, `hyper-vibekanban-${version}.tgz`);
  execFileSync('tar', ['-czf', tgz, 'package'], { cwd: dir });
  return tgz;
}

function writeNpmStub(binDir) {
  const stub = path.join(binDir, 'npm');
  fs.writeFileSync(
    stub,
    `#!/bin/sh
printf '%s\\n' "$*" >> "$STUB_LOG"
printf 'NPM_TOKEN=%s\\n' "$NPM_TOKEN" >> "$STUB_LOG"
printf 'NODE_AUTH_TOKEN=%s\\n' "$NODE_AUTH_TOKEN" >> "$STUB_LOG"
case "$1" in
  --version) echo 11.12.0 ;;
  view)
    if [ "$STUB_MODE" = "exists" ]; then
      echo 0.1.45
      exit 0
    fi
    echo "npm error code E404" >&2
    echo "No match found for version" >&2
    exit 1
    ;;
  publish) exit 0 ;;
  *) exit 1 ;;
esac
`
  );
  fs.chmodSync(stub, 0o755);
  return stub;
}

test('publish script uses OIDC args and does not forward npm tokens', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-npm-publish-'));
  const home = path.join(dir, 'home');
  const binDir = path.join(dir, 'bin');
  fs.mkdirSync(home);
  fs.mkdirSync(binDir);
  fs.writeFileSync(
    path.join(home, '.npmrc'),
    '//registry.npmjs.org/:_authToken=${NODE_AUTH_TOKEN}\n'
  );
  writeNpmStub(binDir);
  const tgz = writeTarball(
    dir,
    '0.1.45',
    'const url = "https://binaries.example.test/vk";\n'
  );
  const log = path.join(dir, 'stub.log');
  const planted = 'npm_test_token_not_real';
  const result = spawnSync(
    process.execPath,
    [path.join(__dirname, 'publish-npm.js'), tgz],
    {
      env: {
        PATH: `${binDir}:/usr/bin:/bin`,
        HOME: home,
        STUB_LOG: log,
        STUB_MODE: 'missing',
        GITHUB_ACTIONS: 'true',
        NPM_TOKEN: planted,
        NODE_AUTH_TOKEN: planted,
      },
      encoding: 'utf8',
    }
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.includes(planted), false);
  assert.equal(result.stderr.includes(planted), false);
  const stubLog = fs.readFileSync(log, 'utf8');
  assert.match(stubLog, /publish .*--provenance --access public --tag latest/);
  assert.match(stubLog, /NPM_TOKEN=\n/);
  assert.equal(stubLog.includes(planted), false);
  const npmrc = fs.readFileSync(path.join(home, '.npmrc'), 'utf8');
  assert.equal(npmrc.includes('_authToken'), false);
  fs.rmSync(dir, { recursive: true, force: true });
});

test('publish script skips a version already on npm', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-npm-skip-'));
  const binDir = path.join(dir, 'bin');
  fs.mkdirSync(binDir);
  writeNpmStub(binDir);
  const tgz = writeTarball(dir, '0.1.45', 'const url = "https://ok.example";\n');
  const log = path.join(dir, 'stub.log');
  const result = spawnSync(
    process.execPath,
    [path.join(__dirname, 'publish-npm.js'), tgz],
    {
      env: {
        PATH: `${binDir}:/usr/bin:/bin`,
        HOME: dir,
        STUB_LOG: log,
        STUB_MODE: 'exists',
        GITHUB_ACTIONS: 'true',
      },
      encoding: 'utf8',
    }
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /already on npm/);
  const stubLog = fs.readFileSync(log, 'utf8');
  assert.equal(stubLog.includes('publish'), false);
  fs.rmSync(dir, { recursive: true, force: true });
});

test('inject CLI writes the file and does not print the URL', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-npm-inject-'));
  const cliPath = path.join(dir, 'cli.js');
  fs.writeFileSync(cliPath, SOURCE);
  const result = spawnSync(
    process.execPath,
    [path.join(__dirname, 'inject-release-config.js'), cliPath],
    {
      env: {
        ...process.env,
        R2_BINARIES_PUBLIC_URL: PUBLIC_URL,
        BINARY_TAG: TAG,
      },
      encoding: 'utf8',
    }
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.includes(PUBLIC_URL), false);
  assert.match(result.stdout, new RegExp(TAG));
  const written = fs.readFileSync(cliPath, 'utf8');
  assert.equal(written.includes(PUBLIC_URL), true);
  assert.equal(written.includes('__R2_PUBLIC_URL__'), false);
  fs.rmSync(dir, { recursive: true, force: true });
});
