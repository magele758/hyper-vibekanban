import { test, expect } from '@playwright/test';
import {
  dismissBlockingDialogs,
  goToLocalWorkspaces,
  openFirstProjectFromSidebar,
  waitForAppReady,
} from '../helpers/app';

/**
 * L4 — visual regression baselines against live Desktop.
 * First run: `pnpm run test:e2e:update-snapshots`
 * Later runs fail on unexpected pixel diffs.
 */
test.describe('visual regression', () => {
  test('project board viewport', async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);
    if (!/\/projects\//.test(page.url())) {
      await openFirstProjectFromSidebar(page);
      await dismissBlockingDialogs(page);
    }
    await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+/);
    // Stabilize dynamic clocks / toasts.
    await page.waitForTimeout(800);
    await expect(page).toHaveScreenshot('project-board.png', {
      fullPage: false,
    });
  });

  test('workspaces list viewport', async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);
    await goToLocalWorkspaces(page);
    await page.waitForTimeout(800);
    // Mask workspace rows — relative timestamps ("2h ago") flake across runs.
    const dynamicRows = page
      .getByRole('button')
      .filter({ hasText: /\d+\s*(m|h|d|w|mo|y|分钟|小时|天|周|月|ago)/i });
    await expect(page).toHaveScreenshot('workspaces-list.png', {
      fullPage: false,
      mask: [dynamicRows],
    });
  });
});
