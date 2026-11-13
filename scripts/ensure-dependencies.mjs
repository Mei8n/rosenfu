import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, existsSync, unlinkSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

// Keep installation outside the build script: CI and manual builds still use npm ci explicitly.
export function ensureDependencies(root, run = spawnSync) {
  const marker = join(root, 'node_modules', '.rosenfu-dependencies');
  const fingerprint = createHash('sha256')
    .update(readFileSync(join(root, 'package.json')))
    .update(readFileSync(join(root, 'package-lock.json')))
    .update(`${process.version}:${process.platform}:${process.arch}`)
    .digest('hex');
  const installedLock = join(root, 'node_modules', '.package-lock.json');
  const installationKey = () => fingerprint + ':' + createHash('sha256')
    .update(readFileSync(installedLock)).digest('hex');
  if (existsSync(marker) && existsSync(installedLock)
      && readFileSync(marker, 'utf8') === installationKey()) return false;

  // Invalidate first so a failed/interrupted install cannot be mistaken for success.
  if (existsSync(marker)) unlinkSync(marker);
  console.log('Rosenfu: 依存関係をロックファイルに同期します (npm ci --include=dev)');
  // npm.cmd needs cmd.exe on Windows. Use one fixed shell command, without
  // an args array concatenated by Node (DEP0190); never interpolate user input.
  const windows = process.platform === 'win32';
  const result = run(windows ? 'npm.cmd ci --include=dev' : 'npm', windows ? [] : ['ci', '--include=dev'], {
    cwd: root, stdio: 'inherit', shell: windows,
  });
  if (result.error || result.status !== 0) {
    throw new Error('依存関係の同期に失敗しました。上の npm エラーを確認してから再実行してください。', { cause: result.error });
  }
  writeFileSync(marker, installationKey());
  return true;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    ensureDependencies(fileURLToPath(new URL('..', import.meta.url)));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
