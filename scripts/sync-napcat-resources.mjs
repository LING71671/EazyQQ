import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.join(root, 'napcat');
const target = path.join(root, 'src-tauri/resources/napcat');
const privateNames = new Set(['config', 'cache', 'logs', 'plugins', '.git', 'loadnapcat.js', 'qqnt.json']);
const within = candidate => candidate.toLowerCase().startsWith(root.toLowerCase() + path.sep);
if (!within(target)) throw new Error('Resource target escaped the workspace');
fs.mkdirSync(target, { recursive: true });
function copy(dir, dest) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const name = entry.name.toLowerCase();
    if (privateNames.has(name) || name.startsWith('eazyqq-') || /\.(db|db-wal|db-shm|sqlite|sqlite3|log|bak)$/.test(name) || name.startsWith('.env') || entry.isSymbolicLink()) continue;
    const from = path.join(dir, entry.name), to = path.join(dest, entry.name);
    if (entry.isDirectory()) { fs.mkdirSync(to, { recursive: true }); copy(from, to); }
    else fs.copyFileSync(from, to);
  }
}
if (fs.existsSync(source)) copy(source, target);
else if (!fs.existsSync(path.join(target, 'napcat.mjs'))) throw new Error('No NapCat runtime resources are available');

// Preserve previous personal resource configuration locally; never ship it in an installer.
const config = path.join(target, 'config');
if (fs.existsSync(config)) {
  const backupParent = path.join(root, '.test-runtime/resource-backups');
  fs.mkdirSync(backupParent, { recursive: true });
  const backup = path.join(backupParent, String(Date.now()));
  if (!within(fs.realpathSync(config)) || !within(fs.realpathSync(backupParent))) throw new Error('Resource backup path escaped the workspace');
  fs.renameSync(config, backup);
}
fs.mkdirSync(config, { recursive: true });
const write = (name, value) => fs.writeFileSync(path.join(config, name), JSON.stringify(value, null, 2) + '\n');
write('webui.json', { host: '127.0.0.1', port: 6099, token: '', autoLoginAccount: '' });
write('napcat.json', { fileLog: true, consoleLog: true, fileLogLevel: 'info', consoleLogLevel: 'info' });
write('onebot11.json', { network: {
  httpServers: [{ name: 'eazyqq-http', enable: true, host: '127.0.0.1', port: 3000, token: '', messagePostFormat: 'array' }],
  websocketServers: [{ name: 'eazyqq-ws', enable: true, host: '127.0.0.1', port: 3001, token: '', messagePostFormat: 'array', reportSelfMessage: false, heartInterval: 30000 }],
  httpClients: [], httpSseServers: [], websocketClients: [], plugins: [],
} });
fs.writeFileSync(path.join(target, 'loadNapCat.js'), 'import("./napcat.mjs");\n');
fs.writeFileSync(path.join(target, 'qqnt.json'), JSON.stringify({ name: 'qq-chat', main: './loadNapCat.js', isPureShell: true, isByteCodeShell: true, platform: 'win32', eleArch: 'x64' }, null, 2) + '\n');
console.log('NapCat resources prepared without account IDs, credentials, local paths or logs');
