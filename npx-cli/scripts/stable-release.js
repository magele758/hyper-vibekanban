#!/usr/bin/env node
'use strict';

// Cut a stable npm tarball from an already published prerelease.
// This file does not call `npm publish`. publish.yml does that.

const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const {
  NPM_PACKAGE_NAME,
  assertPrereleaseDistTag,
  assertReleaseTag,
  selectPrereleaseTag,
  stableReleaseTag,
  stableVersionFromPrerelease,
} = require('./npm-release');

const WORKSPACE_VERSION_FILES = [
  'package.json',
  'npx-cli/package.json',
  'npx-cli/package-lock.json',
  'packages/local-web/package.json',
];

function readPackedJson(tgzPath) {
  const raw = execFileSync('tar', ['-xOf', tgzPath, 'package/package.json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  return JSON.parse(raw);
}

function replaceVersionFields(text, fromVersion, toVersion) {
  const escaped = fromVersion.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`("version"\\s*:\\s*")${escaped}(")`, 'g');
  let count = 0;
  const next = text.replace(pattern, (_match, prefix, suffix) => {
    count += 1;
    return `${prefix}${toVersion}${suffix}`;
  });
  return { text: next, count };
}

function cutStableTarball(sourceTgz, destDir) {
  if (!sourceTgz || !fs.existsSync(sourceTgz)) {
    throw new Error('Prerelease tarball is missing.');
  }
  const manifest = readPackedJson(sourceTgz);
  if (manifest.name !== NPM_PACKAGE_NAME) {
    throw new Error(
      `Refusing to cut a package that is not ${NPM_PACKAGE_NAME}.`
    );
  }
  if (manifest.private === true) {
    throw new Error('Refusing to cut a stable release from a private package.');
  }
  const version = stableVersionFromPrerelease(manifest.version);
  const tag = stableReleaseTag(version);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-cut-stable-'));
  const outPath = path.resolve(destDir, `${NPM_PACKAGE_NAME}-${version}.tgz`);
  try {
    execFileSync('tar', ['-xzf', path.resolve(sourceTgz), '-C', tmp], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const pkgPath = path.join(tmp, 'package', 'package.json');
    const original = fs.readFileSync(pkgPath, 'utf8');
    const replaced = replaceVersionFields(original, manifest.version, version);
    if (replaced.count < 1) {
      throw new Error('Packed package.json has no version field.');
    }
    fs.writeFileSync(pkgPath, replaced.text);
    fs.mkdirSync(destDir, { recursive: true });
    execFileSync('tar', ['-czf', outPath, 'package'], {
      cwd: tmp,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
  if (!path.isAbsolute(outPath)) {
    throw new Error('Stable tarball path must be absolute.');
  }
  return {
    sourceVersion: manifest.version,
    version,
    tag,
    tarball: outPath,
  };
}

function writeWorkspaceVersions(version, root) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Refusing to record non-stable version ${version}.`);
  }
  const files = [];
  for (const rel of WORKSPACE_VERSION_FILES) {
    const full = path.join(root, rel);
    if (!fs.existsSync(full)) {
      if (rel.endsWith('package-lock.json')) continue;
      throw new Error(`Missing ${rel}.`);
    }
    const original = fs.readFileSync(full, 'utf8');
    const current = JSON.parse(original).version;
    if (current === version) continue;
    const replaced = replaceVersionFields(original, current, version);
    if (replaced.count < 1) {
      throw new Error(`No version field updated in ${rel}.`);
    }
    if (replaced.text === original) {
      throw new Error(`Version replace did not change ${rel}.`);
    }
    fs.writeFileSync(full, replaced.text);
    files.push(rel);
  }
  return files;
}

function readReleases(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function main() {
  const [cmd, ...args] = process.argv.slice(2);
  if (cmd === 'select') {
    const chosen = selectPrereleaseTag(readReleases(args[0]));
    process.stdout.write(`${assertReleaseTag(chosen.tagName)}\n`);
    return;
  }
  if (cmd === 'check-tag') {
    process.stdout.write(`${assertReleaseTag(args[0])}\n`);
    return;
  }
  if (cmd === 'assert-prerelease') {
    const manifest = readPackedJson(args[0]);
    const tag = assertPrereleaseDistTag(manifest.version);
    console.log(`${manifest.version} dist-tag ${tag}`);
    return;
  }
  if (cmd === 'cut') {
    const plan = cutStableTarball(args[0], args[1]);
    process.stdout.write(
      [
        `source_version=${plan.sourceVersion}`,
        `stable_version=${plan.version}`,
        `stable_tag=${plan.tag}`,
        `tarball=${plan.tarball}`,
        '',
      ].join('\n')
    );
    return;
  }
  if (cmd === 'write-versions') {
    const files = writeWorkspaceVersions(args[0], args[1] || process.cwd());
    process.stdout.write(files.length === 0 ? '' : `${files.join('\n')}\n`);
    return;
  }
  console.error(
    'Usage: node scripts/stable-release.js ' +
      '<select|check-tag|assert-prerelease|cut|write-versions>'
  );
  process.exit(1);
}

if (require.main === module) {
  try {
    main();
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
}

module.exports = {
  WORKSPACE_VERSION_FILES,
  cutStableTarball,
  replaceVersionFields,
  writeWorkspaceVersions,
};
