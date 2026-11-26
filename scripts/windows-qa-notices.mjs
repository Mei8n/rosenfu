// Generate a versioned, conservative dependency notice inventory for Windows QA.
// Build-only tools are excluded here; installer/native-vendor notices are audited separately.
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';

const base = resolve(import.meta.dirname, '..');
const [destination] = process.argv.slice(2);
if (!destination || process.argv.length !== 3) throw Error('Usage: node scripts/windows-qa-notices.mjs DESTINATION');
const out = resolve(destination);
if (existsSync(out)) throw Error('Notice destination must be new');
const target = 'x86_64-pc-windows-msvc';
const cargo = args => execFileSync('cargo', args, { cwd: base, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
const meta = JSON.parse(cargo(['metadata', '--locked', '--filter-platform', target, '--format-version', '1']));
const tree = cargo(['tree','--locked','-p','rosenfu-desktop','--target',target,'--features','tauri/custom-protocol','-e','normal,no-proc-macro','--prefix','none','--format','{p}']);
const crates = new Map();
const byVersion = new Map();
for (const pkg of meta.packages) {
  const key = `${pkg.name} v${pkg.version}`;
  if (byVersion.has(key)) throw Error(`Ambiguous Cargo metadata for ${key}`);
  byVersion.set(key, pkg);
}
for (const line of tree.trim().split(/\r?\n/)) {
  const display = line.match(/^([\w-]+ v[^\s]+)(?: .*)?$/)?.[1];
  const pkg = byVersion.get(display);
  if (!pkg) throw Error(`Unrecognized cargo tree entry: ${line}`);
  if (pkg.source) {
    if (pkg.source !== 'registry+https://github.com/rust-lang/crates.io-index') throw Error(`Unreviewed crate source: ${pkg.source}`);
    crates.set(display, pkg);
  }
}

const overrides = {
  'alloc-stdlib v0.3.0': ['alloc-stdlib-0.3.0.txt', 'https://github.com/dropbox/rust-alloc-no-stdlib/blob/0a81fd6928ea3b33c8cd484aa4575d50ffb98012/LICENSE'],
  'webview2-com v0.39.1': ['webview2-rs-0.39.1.txt', 'https://github.com/wravery/webview2-rs/blob/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE'],
  'webview2-com-sys v0.39.1': ['webview2-rs-0.39.1.txt', 'https://github.com/wravery/webview2-rs/blob/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE'],
};
const namedNotice = /^(?:licen[cs]es?|notice|copying|copyright)(?:[._ -]|$)/i;
function notices(directory, nested = false) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    if (entry.isFile() && namedNotice.test(entry.name)) return [path];
    if (entry.isDirectory() && (nested || namedNotice.test(entry.name))) return notices(path, true);
    return [];
  });
}
function copyNoticeSet(list, sourceDir, destDir) {
  mkdirSync(destDir, { recursive: true });
  for (const file of list) {
    const dest = join(destDir, relative(sourceDir, file));
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(file, dest);
  }
}
const index = [];
const mpl = [];
for (const [key, pkg] of [...crates].sort(([a],[b]) => a.localeCompare(b, 'en'))) {
  if (!pkg.license && !pkg.license_file) throw Error(`Missing license metadata: ${key}`);
  const source = dirname(pkg.manifest_path);
  const files = notices(source, ['aws-lc-sys','aws-lc-rs','ring','webview2-com-sys'].includes(pkg.name));
  if (pkg.license_file) {
    const license = resolve(source, pkg.license_file);
    if (!license.startsWith(source + (process.platform === 'win32' ? '\\' : '/'))) throw Error(`License outside crate: ${key}`);
    if (!files.includes(license)) files.push(license);
  }
  const dest = join(out, 'rust', `${pkg.name}-${pkg.version}`);
  const upstream = overrides[key];
  if (files.length) copyNoticeSet(files, source, dest);
  else if (upstream) copyNoticeSet([join(import.meta.dirname, 'windows-qa-notice-overrides', upstream[0])], join(import.meta.dirname, 'windows-qa-notice-overrides'), dest);
  else throw Error(`Missing full license/notice text: ${key}`);
  const crateUrl = `https://crates.io/api/v1/crates/${pkg.name}/${pkg.version}/download`;
  index.push({ ecosystem: 'cargo', name: pkg.name, version: pkg.version, license: pkg.license, corresponding_source: crateUrl, upstream_notice: !files.length ? upstream[1] : undefined, files: files.length ? files.map(file => relative(source,file).replaceAll('\\','/')).sort() : [upstream[0]] });
  if (pkg.license?.includes('MPL-2.0')) mpl.push(`- ${key}: ${crateUrl}`);
}

const npm = JSON.parse(readFileSync(join(base,'package-lock.json'),'utf8'));
for (const [key, entry] of Object.entries(npm.packages).sort()) {
  if (!key || entry.dev) continue;
  if (!/^node_modules\/(?:@[\w.-]+\/)?[\w.-]+(?:\/node_modules\/(?:@[\w.-]+\/)?[\w.-]+)*$/.test(key) || key.split('/').some(segment => segment === '..' || segment === '.')) throw Error(`Invalid npm package path: ${key}`);
  const source = join(base,key);
  const pkg = JSON.parse(readFileSync(join(source,'package.json'),'utf8'));
  if (pkg.version !== entry.version || !pkg.license) throw Error(`Invalid npm package metadata: ${key}`);
  const files = notices(source);
  if (!files.length) throw Error(`Missing npm license/notice text: ${key}`);
  copyNoticeSet(files,source,join(out,'npm',key));
  index.push({ ecosystem: 'npm-production', name: pkg.name, version: pkg.version, license: pkg.license, path: key, files: files.map(file => relative(source,file).replaceAll('\\','/')).sort() });
}

writeFileSync(join(out,'DEPENDENCIES.json'), JSON.stringify({target, packages:index},null,2)+'\n');
writeFileSync(join(out,'README.txt'), [
  'Rosenfu Windows x64 QA: third-party application software notices',
  'Rust: package build graph for Windows, excluding development, build-only and proc-macro dependencies.',
  'npm: conservative production lockfile set; some listed files may not be bundled.',
  'This does not relicense Rosenfu, cover third-party installer components, or change geodata terms.',
  '', 'MPL-2.0 covered packages: corresponding unmodified crate source is available from these versioned URLs:',
  ...mpl, '',
].join('\n'));
console.log(`Staged ${crates.size} Rust and ${index.length - crates.size} npm dependency notice sets`);
