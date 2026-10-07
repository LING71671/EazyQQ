import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

const srcDir = path.join(rootDir, 'napcat');
const destDir = path.join(rootDir, 'src-tauri', 'resources', 'napcat');

console.log(`[sync-napcat] Syncing NapCat runtime from ${srcDir} to ${destDir}...`);

if (!fs.existsSync(srcDir)) {
  if (fs.existsSync(destDir)) {
    console.log(`[sync-napcat] Root napcat/ not found, but pre-bundled resources exist at ${destDir}. Skipping sync.`);
    process.exit(0);
  }
  console.error(`[sync-napcat] Source directory not found: ${srcDir}`);
  process.exit(1);
}

fs.mkdirSync(destDir, { recursive: true });

const IGNORE_PATTERNS = [
  /cache/,
  /\.log$/,
  /guild1\.db/,
  /\.db-shm$/,
  /\.db-wal$/,
  /\.git/,
];

function shouldCopy(relPath) {
  for (const pattern of IGNORE_PATTERNS) {
    if (pattern.test(relPath)) return false;
  }
  return true;
}

function copyRecursive(src, dest, rel = '') {
  const entries = fs.readdirSync(src, { withFileTypes: true });
  for (const entry of entries) {
    const entryRel = rel ? `${rel}/${entry.name}` : entry.name;
    if (!shouldCopy(entryRel)) continue;

    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      fs.mkdirSync(destPath, { recursive: true });
      copyRecursive(srcPath, destPath, entryRel);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

copyRecursive(srcDir, destDir);
console.log(`[sync-napcat] NapCat runtime successfully synced to ${destDir}.`);
