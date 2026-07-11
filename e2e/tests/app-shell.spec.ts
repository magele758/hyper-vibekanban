import { test, expect } from '@playwright/test';
import {
  dismissBlockingDialogs,
  goToLocalWorkspaces,
  waitForAppReady,
} from '../helpers/app';

/**
 * L3 smoke — shell + navigation against live Desktop (:13001).
 */
test.describe('app shell smoke', () => {
  test('loads and lands on a project or workspaces route', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await waitForAppReady(page);
    await dismissBlockingDialogs(page);

    await expect(
      page.getByRole('button', { name: 'Open Command Bar' }).first()
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Local workspaces' }).first()
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Settings' }).first()
    ).toBeVisible();

    // Ignore noisy third-party / HMR noise; fail on React crash signatures.
    const fatal = consoleErrors.filter(
      (e) =>
        /Minified React error|Uncaught|TypeError:|ReferenceError:/i.test(e) &&
        !/ResizeObserver|favicon|posthog|sentry/i.test(e)
    );
    expect(fatal, fatal.join('\n')).toEqual([]);
  });

  test('navigates to Local workspaces list', async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);
    await goToLocalWorkspaces(page);

    await expect(
      page.getByRole('button', { name: /Sort workspaces|Filter workspaces/ }).first()
    ).toBeVisible({ timeout: 20_000 });
  });

  test('opens command bar with keyboard', async ({ page }) => {
    await waitForAppReady(page);
    await dismissBlockingDialogs(page);

    await page.keyboard.press('Meta+k');
    // Command bar dialog / combobox should appear.
    const dialog = page.getByRole('dialog').or(page.getByRole('combobox'));
    await expect(dialog.first()).toBeVisible({ timeout: 10_000 });
    await page.keyboard.press('Escape');
  });
});
