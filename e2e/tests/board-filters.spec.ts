import { test, expect } from '@playwright/test';
import {
  closeSettings,
  dismissBlockingDialogs,
  ensureProjectBoard,
  openSettings,
} from '../helpers/app';

/**
 * P0 — board filters / view toggles / settings against live Desktop.
 */
test.describe('board filters & settings', () => {
  test.beforeEach(async ({ page }) => {
    await ensureProjectBoard(page);
  });

  test('toggles 活动 / 全部 / Backlog filters without crash', async ({
    page,
  }) => {
    for (const name of ['活动', '全部', 'Backlog', 'Cancelled']) {
      const btn = page.getByRole('button', { name, exact: true }).first();
      await expect(btn).toBeVisible();
      await btn.click();
      await page.waitForTimeout(300);
      await expect(
        page.getByRole('button', { name: /新问题|New issue/i }).first()
      ).toBeVisible();
    }
    // Return to active board.
    await page.getByRole('button', { name: '活动', exact: true }).click();
  });

  test('toggles Team / Personal views', async ({ page }) => {
    await page.getByRole('button', { name: 'Personal', exact: true }).click();
    await page.waitForTimeout(300);
    await expect(
      page.getByRole('button', { name: /新问题|New issue/i }).first()
    ).toBeVisible();
    await page.getByRole('button', { name: 'Team', exact: true }).click();
    await page.waitForTimeout(300);
    await expect(
      page.getByRole('button', { name: /新问题|New issue/i }).first()
    ).toBeVisible();
  });

  test('opens Filters popover', async ({ page }) => {
    await page.getByRole('button', { name: 'Filters' }).first().click();
    // Popover / dialog should mount something interactive.
    const panel = page
      .getByRole('dialog')
      .or(page.getByRole('menu'))
      .or(page.getByRole('listbox'));
    await expect(panel.first()).toBeVisible({ timeout: 10_000 });
    await page.keyboard.press('Escape');
    await dismissBlockingDialogs(page);
  });

  test('opens Settings and switches sections', async ({ page }) => {
    await openSettings(page);
    for (const section of ['常规', '仓库', '代理', '组织设置', '项目']) {
      const tab = page.getByRole('button', { name: section, exact: true });
      if (await tab.isVisible().catch(() => false)) {
        await tab.click();
        await page.waitForTimeout(200);
      }
    }
    // Theme / language controls exist in General.
    await page.getByRole('button', { name: '常规', exact: true }).click().catch(() => undefined);
    await expect(
      page
        .getByRole('button', { name: /Light|Dark|简体中文|English/i })
        .first()
    ).toBeVisible({ timeout: 10_000 });
    await closeSettings(page);
    await expect(
      page.getByRole('button', { name: /新问题|New issue/i }).first()
    ).toBeVisible();
  });

  test('Account button is reachable', async ({ page }) => {
    await page.getByRole('button', { name: 'Account' }).first().click();
    await page.waitForTimeout(400);
    // Menu or dialog; Escape to dismiss either way.
    await page.keyboard.press('Escape');
    await dismissBlockingDialogs(page);
    await expect(
      page.getByRole('button', { name: 'Settings' }).first()
    ).toBeVisible();
  });
});
