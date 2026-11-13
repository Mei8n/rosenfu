import { afterEach, describe, expect, it, vi } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ensureDependencies } from './ensure-dependencies.mjs';

const roots = [];
afterEach(() => { vi.unstubAllGlobals(); for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'rosenfu-deps-'));
  roots.push(root);
  writeFileSync(join(root, 'package.json'), '{}');
  writeFileSync(join(root, 'package-lock.json'), '{}');
  const run = vi.fn(() => {
    mkdirSync(join(root, 'node_modules'), { recursive: true });
    writeFileSync(join(root, 'node_modules', '.package-lock.json'), '{}');
    return { status: 0 };
  });
  return { root, run };
}
describe('debug dependency synchronization', () => {
  it('installs dev dependencies on first launch and skips unchanged later launches', () => {
    const { root, run } = fixture();
    expect(ensureDependencies(root, run)).toBe(true);
    expect(run).toHaveBeenCalledWith(expect.any(String), expect.any(Array), expect.objectContaining({ cwd: root }));
    expect(ensureDependencies(root, run)).toBe(false);
    expect(run).toHaveBeenCalledTimes(1);
  });
  it.each(['win32', 'linux', 'darwin'])('uses a fixed npm command on %s', (platform) => {
    vi.stubGlobal('process', { ...process, platform });
    const { root, run } = fixture();
    ensureDependencies(root, run);
    const windows = platform === 'win32';
    expect(run).toHaveBeenCalledWith(
      windows ? 'npm.cmd ci --include=dev' : 'npm',
      windows ? [] : ['ci', '--include=dev'],
      { cwd: root, stdio: 'inherit', shell: windows },
    );
  });
  it.each(['package.json', 'package-lock.json'])('resynchronizes after pulling a changed %s', (file) => {
    const { root, run } = fixture();
    ensureDependencies(root, run);
    writeFileSync(join(root, file), '{"changed":true}');
    expect(ensureDependencies(root, run)).toBe(true);
    expect(run).toHaveBeenCalledTimes(2);
  });
  it('resynchronizes if a separate install changes the installed dependency graph', () => {
    const { root, run } = fixture();
    ensureDependencies(root, run);
    writeFileSync(join(root, 'node_modules', '.package-lock.json'), '{"omitDev":true}');
    expect(ensureDependencies(root, run)).toBe(true);
  });
  it('reinstalls after node_modules is deleted', () => {
    const { root, run } = fixture();
    ensureDependencies(root, run);
    rmSync(join(root, 'node_modules'), { recursive: true });
    expect(ensureDependencies(root, run)).toBe(true);
  });
  it('does not stamp a failed install and retries on the next launch', () => {
    const { root, run } = fixture();
    ensureDependencies(root, run);
    writeFileSync(join(root, 'package-lock.json'), '{"changed":true}');
    expect(() => ensureDependencies(root, () => ({ status: 1 }))).toThrow('同期に失敗');
    expect(existsSync(join(root, 'node_modules', '.rosenfu-dependencies'))).toBe(false);
    expect(ensureDependencies(root, run)).toBe(true);
  });
  it('fails safely if npm cannot be launched', () => {
    const { root } = fixture();
    expect(() => ensureDependencies(root, () => ({ error: new Error('ENOENT'), status: null }))).toThrow('同期に失敗');
    expect(existsSync(join(root, 'node_modules', '.rosenfu-dependencies'))).toBe(false);
  });
  it('keeps F5 dependency preparation before desktop compilation', () => {
    const tasks = JSON.parse(readFileSync(new URL('../.vscode/tasks.json', import.meta.url), 'utf8')).tasks;
    expect(tasks.find(t => t.label === 'Rosenfu: build debug desktop').dependsOn).toEqual(['Rosenfu: prepare dependencies']);
    expect(tasks.find(t => t.label === 'Rosenfu: prepare dependencies')).toMatchObject({ command: 'node', args: ['scripts/ensure-dependencies.mjs'] });
  });
});
