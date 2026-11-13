import { expect, test } from 'vitest';

// Extensionless imports must have one unambiguous source stem even on the
// case-insensitive filesystems commonly used by Windows and macOS.
const sourceModules = import.meta.glob('./**/*.{ts,tsx,js,jsx}', { query: '?raw', import: 'default' });

test('source module stems remain distinct on case-insensitive filesystems', () => {
  const pathsByStem = new Map<string, string[]>();
  for (const path of Object.keys(sourceModules)) {
    const stem = path.replace(/\.(?:tsx?|jsx?)$/, '').toLowerCase();
    pathsByStem.set(stem, [...(pathsByStem.get(stem) ?? []), path]);
  }
  expect([...pathsByStem.values()].filter(paths => paths.length > 1)).toEqual([]);
});
