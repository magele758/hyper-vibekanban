import { expect, type Page } from '@playwright/test';

/** Wait until SPA leaves the root loading splash. */
export async function waitForAppReady(page: Page) {
  await page.goto('/');
  await expect(page.getByText('Loading...')).toHaveCount(0, {
    timeout: 30_000,
  });
  await expect(page).toHaveURL(/\/(projects\/|workspaces)/, {
    timeout: 30_000,
  });
}

export async function dismissBlockingDialogs(page: Page) {
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press('Escape');
    await page.waitForTimeout(150);
  }
}

export async function goToLocalWorkspaces(page: Page) {
  const link = page.getByRole('button', { name: 'Local workspaces' }).first();
  await expect(link).toBeVisible({ timeout: 20_000 });
  await link.click();
  await expect(page).toHaveURL(/\/workspaces/, { timeout: 20_000 });
}

export async function openFirstProjectFromSidebar(page: Page) {
  const project = page
    .getByRole('button', { name: /Initial Project|Create project/ })
    .filter({ hasNotText: 'Create project' })
    .first();
  await expect(project).toBeVisible({ timeout: 20_000 });
  await project.click();
  await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+/, { timeout: 20_000 });
}

/** Ensure we are on a project kanban board (not workspaces / issue-only). */
export async function ensureProjectBoard(page: Page) {
  await waitForAppReady(page);
  await dismissBlockingDialogs(page);
  if (!/\/projects\/[0-9a-f-]+/.test(page.url())) {
    await openFirstProjectFromSidebar(page);
    await dismissBlockingDialogs(page);
  }
  // Strip issue deep-link to board root if needed.
  const m = page.url().match(/(\/projects\/[0-9a-f-]+)/);
  if (m && /\/issues\//.test(page.url())) {
    await page.goto(m[1]!);
    await dismissBlockingDialogs(page);
  }
  await expect(
    page.getByRole('button', { name: /新问题|New issue/i }).first()
  ).toBeVisible({ timeout: 20_000 });
}

export async function openNewIssueComposer(page: Page) {
  const btn = page.getByRole('button', { name: /新问题|New issue/i }).first();
  await btn.click();
  await expect(page.getByRole('textbox', { name: 'Issue title' })).toBeVisible({
    timeout: 15_000,
  });
  // Exact label — /Create/i also matches "Create project".
  await expect(page.getByRole('button', { name: '创建任务' })).toBeVisible();
}

export async function createIssueOnBoard(
  page: Page,
  title: string
): Promise<void> {
  // Clear any persisted search — optimistic issues without simple_id used to
  // crash the board filter when a query was active.
  const search = page.getByPlaceholder(/Search issues|搜索/i).first();
  if (await search.isVisible().catch(() => false)) {
    await search.fill('');
    await page.waitForTimeout(200);
  }

  await openNewIssueComposer(page);
  const titleBox = page.getByRole('textbox', { name: 'Issue title' });
  await titleBox.click();
  await titleBox.fill(title);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(200);

  const createBtn = page.getByRole('button', { name: '创建任务' });
  await expect(createBtn).toBeEnabled({ timeout: 10_000 });
  await createBtn.click();

  const errorBanner = page.getByText('Something went wrong!');
  if (await errorBanner.isVisible({ timeout: 2_000 }).catch(() => false)) {
    const detail = await page.locator('code').first().innerText().catch(() => '');
    throw new Error(`Issue create crashed UI: ${detail || 'Something went wrong!'}`);
  }

  await expect(
    page.getByRole('button', { name: new RegExp(escapeRegExp(title)) }).first()
  ).toBeVisible({ timeout: 25_000 });
}

export async function openIssueCard(page: Page, title: string) {
  const card = page
    .getByRole('button', { name: new RegExp(escapeRegExp(title)) })
    .first();
  await card.click();
  await expect(page.getByRole('textbox', { name: 'Issue title' })).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByRole('button', { name: /关闭面板|Close/i })).toBeVisible();
}

export async function closeSidePanel(page: Page) {
  const close = page.getByRole('button', { name: /关闭面板|Close/i }).first();
  if (await close.isVisible().catch(() => false)) {
    await close.click();
  } else {
    await page.keyboard.press('Escape');
  }
  await page.waitForTimeout(200);
}

export async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings' }).first().click();
  await expect(
    page.getByRole('button', { name: /常规|General|关闭|Close/i }).first()
  ).toBeVisible({ timeout: 15_000 });
}

export async function closeSettings(page: Page) {
  const close = page.getByRole('button', { name: /关闭|Close/i }).last();
  if (await close.isVisible().catch(() => false)) {
    await close.click();
  } else {
    await page.keyboard.press('Escape');
  }
  await page.waitForTimeout(200);
}

export function uniqueE2ETitle(prefix = 'e2e'): string {
  return `${prefix}-${Date.now()}-${Math.floor(Math.random() * 1e4)}`;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
