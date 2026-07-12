import { test, expect } from '@playwright/test';
import {
  closeSidePanel,
  createIssueOnBoard,
  dismissBlockingDialogs,
  ensureProjectBoard,
  openIssueCard,
  uniqueE2ETitle,
} from '../helpers/app';

/**
 * P0 truth — full issue path in one test (avoids serial state + leftover overlays).
 */
test.describe('issue CRUD truth', () => {
  test('create → search → open → rename', async ({ page }) => {
    await ensureProjectBoard(page);
    // Ensure active board filter (not Backlog/Cancelled empty views).
    await page.getByRole('button', { name: '活动', exact: true }).click();
    await page.getByRole('button', { name: 'Team', exact: true }).click();
    await dismissBlockingDialogs(page);

    const title = uniqueE2ETitle('e2e-create');
    await createIssueOnBoard(page, title);
    await closeSidePanel(page);
    await dismissBlockingDialogs(page);

    await expect(
      page.getByRole('button', { name: new RegExp(title) }).first()
    ).toBeVisible();

    const search = page.getByPlaceholder(/Search issues|搜索/i).first();
    await search.fill(title);
    await expect(
      page.getByRole('button', { name: new RegExp(title) }).first()
    ).toBeVisible({ timeout: 15_000 });
    await search.fill('__no_such_issue_zzz__');
    await page.waitForTimeout(400);
    await expect(
      page.getByRole('button', { name: new RegExp(title) })
    ).toHaveCount(0);
    await search.fill('');
    await page.waitForTimeout(300);

    await openIssueCard(page, title);
    const titleBox = page.getByRole('textbox', { name: 'Issue title' });
    await expect(titleBox).toHaveValue(title);
    await expect(
      page.getByRole('button', { name: /To do|Todo|In progress/i }).first()
    ).toBeVisible();

    const renamed = uniqueE2ETitle('e2e-renamed');
    await titleBox.click();
    await titleBox.fill(renamed);
    await titleBox.press('Tab');
    await page.waitForTimeout(1000);
    await closeSidePanel(page);
    await dismissBlockingDialogs(page);

    await expect(
      page.getByRole('button', { name: new RegExp(renamed) }).first()
    ).toBeVisible({ timeout: 20_000 });
  });
});
