#!/usr/bin/env node
'use strict';

const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const {
  npmSupportsOidc,
  planPublish,
  npmViewIsMissing,
  stripEmptyNpmAuth,
  buildPublishArgs,
} = require('./npm-release');

const TOKEN_ENV_KEYS = [
  'NPM_TOKEN',
  'NODE_AUTH_TOKEN',
  'NPM_AUTH_TOKEN',
  'NPM_CONFIG__AUTH',
  'npm_config__auth',
];

function dropNpmTokens() {
  for (const key of TOKEN_ENV_KEYS) {
    delete process.env[key];
  }
}

function npmrcPaths() {
  const paths = [
    path.join(os.homedir(), '.npmrc'),
    path.join(process.cwd(), '.npmrc'),
  ];
  if (process.env.RUNNER_TEMP) {
    paths.push(path.join(process.env.RUNNER_TEMP, '.npmrc'));
  }
  if (process.env.GITHUB_WORKSPACE) {
    paths.push(path.join(process.env.GITHUB_WORKSPACE, '.npmrc'));
  }
  return paths;
}

function stripEmptyAuthFiles() {
  // setup-node writes an empty auth line into ~/.npmrc. Only touch that on
  // the GitHub runner; a local npmrc is left alone.
  if (process.env.GITHUB_ACTIONS !== 'true') return;
  let removed = false;
  for (const filePath of npmrcPaths()) {
    if (!fs.existsSync(filePath)) continue;
    const original = fs.readFileSync(filePath, 'utf8');
    const next = stripEmptyNpmAuth(original);
    if (next === original) continue;
    fs.writeFileSync(filePath, next.endsWith('\n') ? next : `${next}\n`);
    removed = true;
  }
  if (removed) {
    console.log('Removed empty npm auth token config so OIDC can be used.');
  }
}

function runNpm(args, inherit) {
  try {
    const stdout = execFileSync('npm', args, {
      encoding: 'utf8',
      stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'pipe'],
      env: process.env,
    });
    return { status: 0, output: inherit ? '' : String(stdout || '') };
  } catch (err) {
    const stdout = err.stdout ? String(err.stdout) : '';
    const stderr = err.stderr ? String(err.stderr) : '';
    return {
      status: typeof err.status === 'number' ? err.status : 1,
      output: `${stdout}\n${stderr}`,
    };
  }
}

function readPackedFile(tgzPath, entry) {
  return execFileSync('tar', ['-xOf', tgzPath, entry], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function main() {
  const tgzPath = process.argv[2];
  if (!tgzPath || !fs.existsSync(tgzPath)) {
    console.error('Usage: node scripts/publish-npm.js <vibe-kanban.tgz>');
    process.exit(1);
  }

  dropNpmTokens();
  stripEmptyAuthFiles();

  const versionResult = runNpm(['--version'], false);
  if (
    versionResult.status !== 0 ||
    !npmSupportsOidc(versionResult.output)
  ) {
    console.error(
      'npm >= 11.5.1 is required for trusted publishing. Refusing to publish.'
    );
    process.exit(1);
  }

  const manifest = JSON.parse(readPackedFile(tgzPath, 'package/package.json'));
  const cliSource = readPackedFile(tgzPath, 'package/bin/cli.js');
  const plan = planPublish({
    name: manifest.name,
    version: manifest.version,
    private: manifest.private,
    cliSource,
  });

  const view = runNpm(
    ['view', `${plan.name}@${plan.version}`, 'version'],
    false
  );
  if (view.status === 0) {
    console.log(
      `${plan.name}@${plan.version} is already on npm; skipping publish.`
    );
    return;
  }
  if (!npmViewIsMissing(view.status, view.output)) {
    const code = (view.output.match(/\bE[A-Z0-9_]+\b/) || [])[0];
    console.error(
      `Could not check the npm registry${
        code ? ` (${code})` : ''
      }. Refusing to publish.`
    );
    process.exit(view.status || 1);
  }

  console.log(
    `Publishing ${plan.name}@${plan.version} with dist-tag ${plan.tag} via OIDC.`
  );
  const published = runNpm(buildPublishArgs(tgzPath, plan.tag), true);
  if (published.status !== 0) {
    console.error(
      'npm publish failed. Trusted publishing for this workflow is missing, ' +
        'or this account cannot publish vibe-kanban.'
    );
    process.exit(published.status || 1);
  }
  console.log(`Published ${plan.name}@${plan.version}.`);
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
  dropNpmTokens,
  stripEmptyAuthFiles,
  TOKEN_ENV_KEYS,
};
