import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const extensions = new Set(['.tsx', '.ts', '.rs', '.mjs', '.js', '.py', '.ps1', '.md', '.txt', '.html', '.css', '.json', '.yml', '.yaml', '.svg']);
const ignored = new Set(['node_modules', 'target', 'resources', '.git', '.test-runtime']);
const forbidden = /[\p{Extended_Pictographic}\uFE0F\u2713\u2717\u2715]|\bSparkles\b/u;
const failures = [];
function scan(relative) {
  for (const item of readdirSync(path.join(root, relative), { withFileTypes: true })) {
    const file = path.join(relative, item.name);
    if (item.isDirectory()) { if (!ignored.has(item.name)) scan(file); continue; }
    if (!extensions.has(path.extname(item.name))) continue;
    readFileSync(path.join(root, file), 'utf8').split(/\r?\n/).forEach((line, index) => {
      if (forbidden.test(line)) failures.push(`${file}:${index + 1}: decorative symbol or star icon`);
    });
  }
}
for (const directory of ['src', 'src-tauri/src', 'scripts', 'docs', '.github']) scan(directory);
for (const file of readdirSync(root)) {
  if (!['.md', '.txt', '.html'].includes(path.extname(file))) continue;
  readFileSync(path.join(root, file), 'utf8').split(/\r?\n/).forEach((line, index) => {
    if (forbidden.test(line)) failures.push(`${file}:${index + 1}: decorative symbol`);
  });
}
if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
else console.log('Presentation check passed: no authored emoji or decorative star icons.');
