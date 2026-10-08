import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8').replaceAll('\r\n', '\n');
const write = process.argv.includes('--write');
const lib = read('src-tauri/src/lib.rs');
const ipc = lib.match(/invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/)[1].split(',').map(value => value.trim()).filter(Boolean).sort();
const main = read('src-tauri/src/bin/eazyqq_cli/main.rs');
const cliModule = read('src-tauri/src/bin/eazyqq_cli/commands/mod.rs');
const cliFiles = new Map([...cliModule.matchAll(/#\[path = "([^"]+)"\]\s*pub mod (\w+);/g)].map(match => [match[2], match[1]]));
const commands = [...read('src-tauri/src/bin/eazyqq_cli/commands/system/schema.rs').matchAll(/^\s+"([a-z-]+)",/gm)].map(match => match[1]);
const route = new Map([...main.matchAll(/"([a-z-]+)"\s*=>\s*commands::(\w+)::(\w+)/g)].map(match => [match[1], [match[2], match[3]]]));
const map = {
  get_protocol_status: 'status', refresh_qrcode: 'qr --refresh', quick_login: 'quick-login', get_quick_login_accounts: 'quick-login-list', logout: 'logout',
  list_accounts: 'accounts list', get_account_status: 'accounts status', register_account: 'accounts add', batch_accounts: 'accounts start|stop|login', account_qrcode: 'accounts qr', configure_account: 'accounts configure', forget_account: 'accounts forget',
  get_contacts: 'contacts', get_chain_status: 'chain-status', repair_chain: 'repair', mark_read: 'mark-read', update_rule: 'rule', batch_update_mode: 'batch-mode',
  get_messages: 'history', send_message: 'send', trigger_ai_reply: 'ask', get_pending_drafts: 'drafts', send_draft: 'draft-send', dismiss_draft: 'draft-dismiss', regenerate_draft: 'draft-regenerate',
  get_group_files: 'files', download_file: 'file-download', summarize_file: 'file-summarize', open_folder: 'folder',
  generate_summary: 'summarize', generate_summary_stream: 'summarize --stream', get_summary_history: 'summaries', delete_summary: 'summary-delete',
  get_config: 'config', update_config: 'set-config', test_ai_connection: 'ai-test', fetch_provider_models: 'ai-models', check_dependencies: 'health', restart_napcat: 'restart', export_diagnostics_bundle: 'export',
  app_minimize_window: 'window minimize', app_toggle_maximize_window: 'window toggle-maximize', app_close_window: 'window close', app_start_drag_window: 'window drag', app_show_window: 'window show', app_get_window_behavior: 'config',
  check_app_update: 'updates check-app', get_qq_path: 'qq-path', set_qq_path: 'qq-path --set', get_napcat_version: 'updates versions', check_napcat_update: 'updates check-napcat', upgrade_napcat: 'updates install-napcat', upgrade_app: 'updates install-app',
};
for (const name of ipc) {
  if (!map[name]) throw new Error(`IPC command has no CLI mapping: ${name}`);
  if (!commands.includes(map[name].split(' ')[0])) throw new Error(`IPC mapping uses an unknown CLI command: ${map[name]}`);
}
const front = read('src/api/client.ts');
for (const [, name] of front.matchAll(/invoke(?:<[^>]*>)?\('([^']+)'/g)) if (!ipc.includes(name)) throw new Error(`Frontend invokes an unregistered command: ${name}`);
const globalFlags = { account: { type: 'string', pattern: '^[1-9][0-9]{4,19}$' }, json: { type: 'boolean' } };
const catalog = commands.map(name => {
  const properties = { ...globalFlags };
  const target = route.get(name);
  if (target && cliFiles.has(target[0])) {
    const source = read(`src-tauri/src/bin/eazyqq_cli/commands/${cliFiles.get(target[0])}`);
    const start = source.indexOf(`fn ${target[1]}(`);
    if (start >= 0) {
      const end = source.indexOf('\npub ', start + 5);
      const body = source.slice(start, end < 0 ? undefined : end);
      for (const [, flag] of body.matchAll(/\.flag\("([^"\n]+)"\)/g)) properties[flag] = { type: 'string' };
      for (const [, flag] of body.matchAll(/\.has\("([^"\n]+)"\)/g)) properties[flag] = { type: 'boolean' };
    }
  }
  if (name === 'folder') properties.path = { type: 'string' };
  return { name, inputSchema: { type: 'object', properties, additionalProperties: false } };
});
const contract = { name: 'eazyqq_cli', version: JSON.parse(read('package.json')).version, globalFlags, commands: catalog,
  ipcCoverage: ipc.map(name => ({ ipc: name, cli: map[name] })),
  accounts: { operations: ['list', 'status', 'add', 'select', 'qr', 'configure', 'forget', 'start', 'stop', 'login'], batchFlags: ['--uin', '--uins', '--all', '--dry-run'], dataDeletion: false },
  updates: { operations: ['versions', 'check-app', 'check-napcat', 'install-app', 'install-napcat'], installRequires: '--confirm' },
  mcp: { transport: 'stdio', discovery: 'tools/list' }, documentation: 'docs/api/CLI.md' };
const schema = JSON.stringify(contract, null, 2) + '\n';
const matrix = '# 桌面接口与 CLI 覆盖矩阵\n\n根据已注册的桌面处理函数和 CLI 路由生成。修改任一适配层后运行 `pnpm contracts:check`；有意变更接口时运行 `pnpm contracts:write` 更新本文。\n\n| 桌面 IPC | CLI 对应操作 |\n| --- | --- |\n' + ipc.map(name => `| \`${name}\` | \`${map[name]}\` |`).join('\n') + '\n';
for (const [file, content] of [['docs/api/cli.schema.json', schema], ['docs/api/COVERAGE.md', matrix]]) {
  if (write) { fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true }); fs.writeFileSync(path.join(root, file), content); }
  else if (read(file) !== content) throw new Error(`Contract drift: ${file}. Run pnpm contracts:write`);
}
console.log(`Contract verified: ${ipc.length} desktop commands, ${commands.length} CLI commands`);
