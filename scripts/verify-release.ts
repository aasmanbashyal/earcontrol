import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '..');
const pkg = await Bun.file(resolve(root, 'desktop/package.json')).json();
const tauri = await Bun.file(resolve(root, 'desktop/src-tauri/tauri.conf.json')).json();
const cargo = await Bun.file(resolve(root, 'desktop/src-tauri/Cargo.toml')).text();
const cargoLock = await Bun.file(resolve(root, 'desktop/src-tauri/Cargo.lock')).text();
const version = pkg.version as string;
const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
const lockedVersion = cargoLock.match(/\[\[package\]\]\s+name = "earcontrol"\s+version = "([^"]+)"/)?.[1];
if (!/^\d+\.\d+\.\d+$/.test(version) || tauri.version !== version || cargoVersion !== version || lockedVersion !== version) {
  throw new Error('Release versions must match package.json, tauri.conf.json, Cargo.toml, and Cargo.lock.');
}
const tag = process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : process.argv.find(arg => arg.startsWith('--tag='))?.slice(6);
if (tag && tag !== `v${version}`) throw new Error(`Tag ${tag} must equal v${version}.`);
if (process.argv.includes('--version-only')) {
  console.log(`Release version ${version} verified.`);
} else {
  const expected = [`EarControl-${version}/EarControl.exe`, `EarControl-${version}-win-x64.zip`, `EarControl_${version}_x64-setup.exe`];
  const manifest = await Bun.file(resolve(root, `dist/EarControl-${version}-SHA256SUMS.txt`)).text();
  const lines = manifest.trim().split(/\r?\n/);
  if (lines.length !== expected.length) throw new Error('Expected exactly three release checksums.');
  for (const name of expected) {
    const line = lines.find(entry => entry.endsWith(`  ${name}`));
    if (!line || !/^[a-f0-9]{64}  /.test(line)) throw new Error(`Missing checksum: ${name}`);
    const path = resolve(root, 'dist', name);
    if ((await stat(path)).size === 0) throw new Error(`Empty artifact: ${name}`);
    const hash = createHash('sha256').update(await readFile(path)).digest('hex');
    if (hash !== line.slice(0, 64)) throw new Error(`Checksum mismatch: ${name}`);
  }
  console.log(`Verified version ${version} and all three release artifacts.`);
}
