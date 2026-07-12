import { test, expect } from '@playwright/test';
import {
  dismissBlockingDialogs,
  goToLocalWorkspaces,
  waitForAppReady,
} from '../helpers/app';

/**
 * P0 — workspaces list truth against live Desktop.
 */
test.describe('workspaces truth', () => {
  test.beforeEach(async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);
    await goToLocalWorkspaces(page);
  });

  test('lists workspaces chrome and search box', async ({ page }) => {
    await expect(page.getByPlaceholder(/搜索|Search/i).first()).toBeVisible();
    await expect(
      page.getByRole('button', { name: /Sort workspaces/i }).first()
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: /Filter workspaces/i }).first()
    ).toBeVisible();
  });

  test('search filters workspace list', async ({ page }) => {
    const search = page.getByPlaceholder(/搜索|Search/i).first();
    await search.fill('__no_ws_match_xyz__');
    await page.waitForTimeout(400);
    // List should not crash; clear.
    await search.fill('');
    await expect(
      page.getByRole('button', { name: /Sort workspaces/i }).first()
    ).toBeVisible();
  });

  test('opens an existing workspace when available', async ({ page }) => {
    // Prefer a named workspace card that is not a section header.
    const candidates = page
      .getByRole('button')
      .filter({ hasText: /\d+\s*(m|h|d|ago|分钟|小时|天)|完整|研究|帮我|Skill/i });
    const count = await candidates.count();
    test.skip(count === 0, 'no workspace cards on this machine');

    const card = candidates.first();
    const label = (await card.innerText()).split('\n')[0]?.trim() ?? '';
    await card.click();
    await expect(page).toHaveURL(/\/workspaces\/[0-9a-f-]+/, {
      timeout: 25_000,
    });
    // Workspace shell chrome.
    await expect(
      page
        .getByRole('button', { name: /Hide Left Sidebar|Toggle Chat|Settings/i })
        .first()
    ).toBeVisible({ timeout: 20_000 });
    test.info().annotations.push({
      type: 'opened',
      description: label.slice(0, 80),
    });
  });

  test('can return to project board from workspaces', async ({ page }) => {
    const project = page
      .getByRole('button', { name: /Initial Project/ })
      .first();
    await expect(project).toBeVisible();
    await project.click();
    await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+/, { timeout: 20_000 });
    await expect(
      page.getByRole('button', { name: /新问题|New issue/i }).first()
    ).toBeVisible();
  });
});
