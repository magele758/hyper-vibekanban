import { defineConfig, devices } from '@playwright/test';
import path from 'node:path';

/**
 * Default: hit the live vk-start main stack on this machine.
 * Override with VK_E2E_* env vars if needed.
 */
const BASE_URL = process.env.VK_E2E_BASE_URL ?? 'http://localhost:13001';

export default defineConfig({
  testDir: path.join(__dirname, 'tests'),
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  timeout: 60_000,
  expect: {
    timeout: 15_000,
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.02,
      animations: 'disabled',
    },
  },
  reporter: [
    ['list'],
    [
      'html',
      { open: 'never', outputFolder: path.join(__dirname, 'playwright-report') },
    ],
  ],
  outputDir: path.join(__dirname, 'test-results'),
  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'off',
    locale: 'zh-CN',
    colorScheme: 'dark',
    viewport: { width: 1440, height: 900 },
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
