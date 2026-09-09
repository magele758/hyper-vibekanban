import { describe, expect, it } from 'vitest';
import { resolveCreateModeBootstrap } from './createModeBootstrap';

describe('resolveCreateModeBootstrap', () => {
  it('treats a local project_id seed as enough to start create mode', async () => {
    const result = await resolveCreateModeBootstrap({
      seedState: { project_id: 'proj-1' },
      isValidProfile: () => false,
    });

    expect(result.source).toBe('seed');
    expect(result.data.localProjectId).toBe('proj-1');
  });
});
