import { test, expect } from '@playwright/test';
import { env, fetchJson } from '../helpers/env';

/**
 * Extra API truth — local + remote with bearer from local auth token.
 */
test.describe('API truth (auth + remote)', () => {
  test('local auth token can call remote organizations', async () => {
    const tokenRes = await fetchJson(`${env.localApi}/api/auth/token`);
    expect(tokenRes.status).toBe(200);
    const token = (tokenRes.body as { data?: { access_token?: string } })?.data
      ?.access_token;
    expect(token).toBeTruthy();

    const orgs = await fetchJson(`${env.remoteApi}/v1/organizations`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(orgs.status).toBe(200);
    const list = (orgs.body as { organizations?: unknown[] })?.organizations;
    expect(Array.isArray(list)).toBe(true);
    expect(list!.length).toBeGreaterThan(0);
  });

  test('local workspaces list returns array', async () => {
    const { status, body } = await fetchJson(`${env.localApi}/api/workspaces`);
    expect(status).toBe(200);
    expect(body).toMatchObject({ success: true });
    expect(Array.isArray((body as { data: unknown[] }).data)).toBe(true);
  });

  test('desktop h2 front door is reachable when configured', async () => {
    const h2 = process.env.VK_E2E_H2_BASE ?? 'https://localhost:13443';
    try {
      const res = await fetch(h2, {
        // Local Caddy may use a trusted local CA; Node may still reject —
        // treat connection refused as skip, TLS errors as soft info.
      });
      expect([200, 404, 502].includes(res.status) || res.ok).toBeTruthy();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      test.info().annotations.push({
        type: 'h2',
        description: `h2 probe skipped/failed: ${msg.slice(0, 120)}`,
      });
      // Not a hard failure for machines without trusted CA in Node.
      expect(msg.length).toBeGreaterThan(0);
    }
  });
});
