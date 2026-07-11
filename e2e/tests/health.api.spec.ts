import { test, expect } from '@playwright/test';
import { env, fetchJson } from '../helpers/env';

/**
 * L1/L2 — stack health against the live vk-start main services.
 * No browser. Fails fast if the target stack is down.
 */
test.describe('stack health (main services)', () => {
  test('local API /api/health', async () => {
    const { ok, status, body } = await fetchJson(`${env.localApi}/api/health`);
    expect(status).toBe(200);
    expect(ok).toBe(true);
    expect(body).toMatchObject({ success: true, data: 'OK' });
  });

  test('local API auth status is logged in', async () => {
    const { status, body } = await fetchJson(`${env.localApi}/api/auth/status`);
    expect(status).toBe(200);
    expect(body).toMatchObject({
      success: true,
      data: { logged_in: true },
    });
  });

  test('local API lists repos', async () => {
    const { status, body } = await fetchJson(`${env.localApi}/api/repos`);
    expect(status).toBe(200);
    expect(body).toMatchObject({ success: true });
    const data = (body as { data: unknown[] }).data;
    expect(Array.isArray(data)).toBe(true);
    expect(data.length).toBeGreaterThan(0);
  });

  test('remote /v1/health', async () => {
    const { ok, status, body } = await fetchJson(`${env.remoteApi}/v1/health`);
    expect(status).toBe(200);
    expect(ok).toBe(true);
    expect(body).toMatchObject({ status: 'ok' });
  });

  test('relay /health', async () => {
    const { ok, status } = await fetchJson(`${env.relayApi}/health`);
    expect(status).toBe(200);
    expect(ok).toBe(true);
  });

  test('desktop web responds', async () => {
    const res = await fetch(env.baseUrl);
    expect(res.status).toBe(200);
    const html = await res.text();
    expect(html).toContain('Vibe Kanban');
  });
});
