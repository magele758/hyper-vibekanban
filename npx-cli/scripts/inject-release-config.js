#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { injectReleaseConfig } = require('./npm-release');

function main() {
  const cliPath =
    process.argv[2] || path.join(__dirname, '..', 'bin', 'cli.js');
  if (!fs.existsSync(cliPath)) {
    console.error(`Missing build output: ${cliPath}`);
    process.exit(1);
  }
  const source = fs.readFileSync(cliPath, 'utf8');
  let next;
  try {
    next = injectReleaseConfig(source, {
      publicUrl: process.env.R2_BINARIES_PUBLIC_URL || '',
      binaryTag: process.env.BINARY_TAG || '',
    });
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
  fs.writeFileSync(cliPath, next);
  console.log(`Injected binary tag ${process.env.BINARY_TAG}`);
}

if (require.main === module) {
  main();
}
