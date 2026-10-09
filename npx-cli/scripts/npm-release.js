'use strict';

// Shared checks for packing and publishing npx-cli. Keep secrets out of
// error messages: callers pass the R2 URL in, and failures report only
// the secret name and the length.

const NPM_PACKAGE_NAME = 'hyper-vibekanban';
const VERSION_RE = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const BINARY_TAG_RE = /^v\d+\.\d+\.\d+[0-9A-Za-z.+-]*$/;
const HTTPS_REST_RE = /^[A-Za-z0-9._~:/?#@!$&()*+,;=%-]+$/;

function isSafeHttpsUrl(value) {
  if (typeof value !== 'string' || value.length === 0 || value.length > 500) {
    return false;
  }
  if (!value.startsWith('https://') || value.includes('__')) {
    return false;
  }
  const rest = value.slice('https://'.length);
  if (!rest || rest.startsWith('/') || !rest.includes('.')) {
    return false;
  }
  return HTTPS_REST_RE.test(rest);
}

function isBinaryTag(value) {
  return (
    typeof value === 'string' &&
    value.length <= 200 &&
    !value.includes('__') &&
    BINARY_TAG_RE.test(value)
  );
}

function injectReleaseConfig(source, { publicUrl, binaryTag }) {
  if (typeof source !== 'string') {
    throw new Error('bin/cli.js build output is missing.');
  }
  if (!isSafeHttpsUrl(publicUrl)) {
    const length = typeof publicUrl === 'string' ? publicUrl.length : 0;
    throw new Error(
      `R2_BINARIES_PUBLIC_URL must be an https URL with no quotes or spaces (length ${length}).`
    );
  }
  if (!isBinaryTag(binaryTag)) {
    throw new Error('BINARY_TAG must look like v0.1.45-20260101120000.');
  }
  if (
    !source.includes('__R2_PUBLIC_URL__') ||
    !source.includes('__BINARY_TAG__')
  ) {
    throw new Error(
      'bin/cli.js is missing __R2_PUBLIC_URL__ or __BINARY_TAG__.'
    );
  }
  const next = source
    .replaceAll('__R2_PUBLIC_URL__', publicUrl)
    .replaceAll('__BINARY_TAG__', binaryTag);
  if (
    next.includes('__R2_PUBLIC_URL__') ||
    next.includes('__BINARY_TAG__')
  ) {
    throw new Error('Release placeholder replacement did not finish.');
  }
  return next;
}

function npmSupportsOidc(version) {
  const parts = String(version)
    .trim()
    .split('.')
    .map((part) => Number(part));
  const [major, minor, patch] = parts;
  if (!Number.isInteger(major) || !Number.isInteger(minor)) return false;
  if (major > 11) return true;
  if (major < 11) return false;
  if (minor > 5) return true;
  if (minor < 5) return false;
  return (patch || 0) >= 1;
}

function distTagForVersion(version) {
  if (!VERSION_RE.test(version)) {
    throw new Error(`Refusing unexpected package version: ${version}`);
  }
  const hyphen = version.indexOf('-');
  if (hyphen === -1) return 'latest';
  const tag = version.slice(hyphen + 1).split('.')[0];
  if (!/^[A-Za-z0-9][A-Za-z0-9-]*$/.test(tag) || tag === 'latest') {
    throw new Error(`Refusing dist-tag for prerelease version ${version}`);
  }
  return tag;
}

function planPublish({ name, version, private: isPrivate, cliSource }) {
  if (name !== NPM_PACKAGE_NAME) {
    throw new Error(
      `Refusing to publish a package that is not ${NPM_PACKAGE_NAME}.`
    );
  }
  if (isPrivate === true) {
    throw new Error('Refusing to publish a private package.');
  }
  if (typeof cliSource !== 'string') {
    throw new Error('Packed bin/cli.js is missing.');
  }
  if (
    cliSource.includes('__R2_PUBLIC_URL__') ||
    cliSource.includes('__BINARY_TAG__')
  ) {
    throw new Error(
      'Refusing to publish: release placeholders are still in bin/cli.js.'
    );
  }
  return {
    name,
    version,
    tag: distTagForVersion(version),
  };
}

function npmViewIsMissing(status, output) {
  if (status === 0) return false;
  return /E404|404 Not Found|No match found for version/i.test(output || '');
}

function stripEmptyNpmAuth(contents) {
  if (typeof contents !== 'string' || contents.length === 0) return contents;
  return contents
    .split('\n')
    .filter((line) => !/_authToken=\$\{NODE_AUTH_TOKEN\}/.test(line))
    .filter((line) => !/_authToken=\s*$/.test(line))
    .join('\n');
}

function buildPublishArgs(tgzPath, tag) {
  return [
    'publish',
    tgzPath,
    '--provenance',
    '--access',
    'public',
    '--tag',
    tag,
  ];
}

module.exports = {
  isSafeHttpsUrl,
  isBinaryTag,
  injectReleaseConfig,
  npmSupportsOidc,
  distTagForVersion,
  planPublish,
  npmViewIsMissing,
  stripEmptyNpmAuth,
  buildPublishArgs,
  NPM_PACKAGE_NAME,
};
