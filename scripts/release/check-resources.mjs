import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const directory = path.join(root, 'src-tauri/resources/protocol', version);
const allowedConfigs = new Set(['webui.json', 'napcat.json', 'onebot11.json']);
let count = 0;
function inspect(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const absolute = path.join(dir, entry.name);
    const relative = path.relative(directory, absolute).replaceAll('\\', '/').toLowerCase();
    if (entry.isSymbolicLink()) throw new Error(`Resource symlink is not permitted: ${relative}`);
    if (/(^|\/)(cache|logs|plugins|\.git)(\/|$)/.test(relative)
        || entry.name.toLowerCase().startsWith('eazyqq-')
        || entry.name.toLowerCase().startsWith('.env')
        || /\.(db|db-wal|db-shm|sqlite|sqlite3|log|bak)$/.test(relative)) {
      throw new Error(`Private or mutable resource cannot be released: ${relative}`);
    }
    if (entry.isDirectory()) inspect(absolute);
    else {
      count++;
      if (relative.startsWith('config/') && !allowedConfigs.has(relative.slice(7))) {
        throw new Error(`Personal protocol configuration cannot be released: ${relative}`);
      }
    }
  }
}
inspect(directory);
for (const file of ['NapCatWinBootMain.exe', 'NapCatWinBootHook.dll', 'napcat.mjs', 'loadNapCat.js', 'qqnt.json']) {
  if (!fs.existsSync(path.join(directory, file))) throw new Error(`Protocol resource is missing: ${file}`);
}
const webui = JSON.parse(fs.readFileSync(path.join(directory, 'config/webui.json'), 'utf8'));
if (webui.token || webui.autoLoginAccount) throw new Error('Protocol resources contain a login credential or selected account');
console.log(`Release resources verified: ${count} files; no personal configuration or mutable cache`);
