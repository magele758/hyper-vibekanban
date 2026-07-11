import { expect, type Page } from '@playwright/test';

/** Wait until SPA leaves the root loading splash. */
export async function waitForAppReady(page: Page) {
  await page.goto('/');
  await expect(page.getByText('Loading...')).toHaveCount(0, {
    timeout: 30_000,
  });
  // Settled on a real app route (project board or workspaces).
  await expect(page).toHaveURL(
    /\/(projects\/|workspaces)/,
    { timeout: 30_000 }
  );
}

export async function dismissBlockingDialogs(page: Page) {
  // Release notes / guides may appear; Escape is usually enough.
  for (let i = 0; i < 2; i++) {
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
  }
}

export async function goToLocalWorkspaces(page: Page) {
  const link = page.getByRole('button', { name: 'Local workspaces' }).first();
  await expect(link).toBeVisible({ timeout: 20_000 });
  await link.click();
  await expect(page).toHaveURL(/\/workspaces/, { timeout: 20_000 });
}

export async function openFirstProjectFromSidebar(page: Page) {
  // Sidebar lists projects; "Initial Project" is the seeded default.
  const project = page
    .getByRole('button', { name: /Initial Project|Create project/ })
    .filter({ hasNotText: 'Create project' })
    .first();
  await expect(project).toBeVisible({ timeout: 20_000 });
  await project.click();
  await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+/, { timeout: 20_000 });
}

export async function assertNoPageErrors(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (err) => {
    errors.push(err.message);
  });
  return () => {
    expect(errors, `pageerrors: ${errors.join('\n')}`).toEqual([]);
  };
}
