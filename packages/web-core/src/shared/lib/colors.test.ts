import { describe, expect, it } from 'vitest';
import { PRESET_COLORS, projectColorFromId } from './colors';

describe('projectColorFromId', () => {
  it('returns a preset color and is stable for the same id', () => {
    const color = projectColorFromId('aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee');
    expect(PRESET_COLORS).toContain(color);
    expect(projectColorFromId('aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee')).toBe(
      color
    );
  });

  it('varies across different ids', () => {
    const a = projectColorFromId('11111111-1111-4111-8111-111111111111');
    const b = projectColorFromId('22222222-2222-4222-8222-222222222222');
    expect(a).not.toBe(b);
  });
});
