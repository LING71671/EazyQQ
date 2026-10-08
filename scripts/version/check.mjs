import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
const tauri = JSON.parse(fs.readFileSync(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
const cargo = fs.readFileSync(path.join(root, 'src-tauri/Cargo.toml'), 'utf8');
const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const lock = fs.readFileSync(path.join(root, 'src-tauri/Cargo.lock'), 'utf8');
const lockedVersion = lock.match(/name = "eazyqq"\r?\nversion = "([^"]+)"/)?.[1];
if (pkg.version !== version || tauri.version !== version || lockedVersion !== version) {
  throw new Error(`Version drift: package=${pkg.version}, Cargo=${version}, Cargo.lock=${lockedVersion}, Tauri=${tauri.version}`);
}
console.log(`Version manifests agree: ${version}`);
