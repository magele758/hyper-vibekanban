import { test, expect } from '@playwright/test';
import {
  dismissBlockingDialogs,
  openFirstProjectFromSidebar,
  waitForAppReady,
} from '../helpers/app';

/**
 * L3 — kanban board critical path on the live main stack.
 */
test.describe('kanban board smoke', () => {
  test.beforeEach(async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);
    if (!/\/projects\//.test(page.url())) {
      await openFirstProjectFromSidebar(page);
      await dismissBlockingDialogs(page);
    }
  });

  test('shows board chrome: search + new issue + columns', async ({ page }) => {
    await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+/);

    await expect(
      page.getByPlaceholder(/Search issues|搜索/i).first()
    ).toBeVisible({ timeout: 20_000 });

    await expect(
      page
        .getByRole('button', { name: /新问题|New issue|Add task/i })
        .first()
    ).toBeVisible();

    // At least one kanban column / status control should be present.
    await expect(
      page.getByRole('button', { name: /Backlog|全部|活动|Add task/i }).first()
    ).toBeVisible();
  });

  test('opens new-issue composer', async ({ page }) => {
    const newIssue = page
      .getByRole('button', { name: /新问题|New issue/i })
      .first();
    await expect(newIssue).toBeVisible({ timeout: 20_000 });
    await newIssue.click();

    // Composer should expose a title/input area.
    const title = page
      .getByRole('textbox')
      .or(page.locator('[contenteditable="true"]'))
      .first();
    await expect(title).toBeVisible({ timeout: 15_000 });
    await page.keyboard.press('Escape');
  });

  test('filters via issue search box', async ({ page }) => {
    const search = page.getByPlaceholder(/Search issues|搜索/i).first();
    await expect(search).toBeVisible();
    await search.fill('__e2e_no_match_xyz__');
    await page.waitForTimeout(500);
    // Board should remain mounted (no crash); clear filter.
    await search.fill('');
    await expect(
      page.getByRole('button', { name: /新问题|New issue|Add task/i }).first()
    ).toBeVisible();
  });
});
