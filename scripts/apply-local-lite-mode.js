#!/usr/bin/env node
/**
 * Plan A: local-only dev — no Remote API, no cloud login.
 * Patches dev_assets/config.json and removes stale OAuth credentials.
 */
const fs = require('fs');
const path = require('path');

const devAssets = path.join(__dirname, '..', 'dev_assets');
const configPath = path.join(devAssets, 'config.json');
const credentialsPath = path.join(devAssets, 'credentials.json');

if (fs.existsSync(configPath)) {
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  config.remote_onboarding_acknowledged = true;
  config.relay_enabled = false;
  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  console.log('Updated dev_assets/config.json for local-lite mode');
} else {
  console.log('No dev_assets/config.json yet — will be created on first run');
}

if (fs.existsSync(credentialsPath)) {
  fs.unlinkSync(credentialsPath);
  console.log('Removed dev_assets/credentials.json (stale cloud login)');
}
