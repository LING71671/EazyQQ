async (page) => {
  await page.setViewportSize({ width: 1200, height: 900 });
  await page.addInitScript(() => {
    const ok = data => ({ success: true, data, timestamp: Date.now() });
    const accounts = [
      { instance: { uin: '10001', nickname: 'Primary account', httpPort: 3000, wsPort: 3001, webuiPort: 6099, autoStart: true }, login: { loggedIn: true, uin: '10001', nickname: 'Primary account', source: 'onebot' }, selected: true },
      { instance: { uin: '10002', nickname: 'Second account', httpPort: 3002, wsPort: 3003, webuiPort: 6100, autoStart: false }, login: { loggedIn: false, source: '' }, selected: false },
    ];
    const links = ['napcat_web_ui', 'qq_login', 'one_bot_http', 'one_bot_ws', 'database', 'ai_provider', 'scheduler', 'frontend'].map((link, index) => ({ link, label: ['NapCat WebUI', 'QQ 登录', 'OneBot HTTP', 'OneBot WebSocket', '本地数据库', '大模型', '定时调度', '前端界面'][index], impact: 'Model generation unavailable', health: link === 'ai_provider' ? 'unknown' : 'ok', detail: link === 'ai_provider' ? 'Configured; inference not yet tested' : 'Verified', last_ok_secs_ago: 0, last_error_secs_ago: null }));
    window.__fixtureCalls = [];
    window.__fixtureAuthenticated = false;
    let currentUin = '10001';
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1, unregisterCallback() {},
      invoke: async (cmd, args = {}) => {
        window.__fixtureCalls.push({ cmd, args });
        if (cmd === 'plugin:event|listen') return 1;
        if (cmd === 'get_protocol_status') return ok({ isConnected: true, loginStatus: 'logged_in', qqNumber: currentUin, nickname: currentUin === '10001' ? 'Primary account' : 'Second account', quickLoginAccounts: [] });
        if (cmd === 'quick_login') {
          if (!window.__fixtureAuthenticated) return { success: false, timestamp: Date.now(), error: { code: 1005, message: 'Quick login expired' } };
          currentUin = args.uin;
          return ok(null);
        }
        if (cmd === 'account_qrcode') return ok({ uin: args.uin, qrcodeBase64: 'https://example.test/login-fixture' });
        if (cmd === 'get_account_status') return ok({ instance: accounts[1].instance, login: { loggedIn: window.__fixtureAuthenticated, uin: '10002', source: 'onebot' }, selected: false });
        if (cmd === 'list_accounts') return ok(accounts);
        if (cmd === 'get_chain_status') return ok({ links, firstBreak: null, hasFailure: false, uptimeSecs: 70 });
        if (cmd === 'get_contacts') return ok({ list: [], total: 0 });
        if (cmd === 'get_config') return ok({ ai: { activeProvider: 'opencode', baseUrl: 'https://opencode.ai/zen/v1', model: 'big-pickle', temperature: 0.7, maxContextMessages: 10 }, napcat: { wsPort: 3001, autoRestart: true, heartbeatIntervalSec: 15 }, storage: { autoSyncFiles: true, maxFileSizeMb: 100 }, summary: { enabled: true, intervalType: '6h', customIntervalMinutes: 360, slidingWindowHours: 6, autoForwardToPhone: false, customPrompt: '' }, window: { closeToTray: true, minimizeToTray: false } });
        if (cmd === 'check_dependencies') return ok({ isAllReady: true, qqNt: { ready: true, path: 'QQ.exe' }, openCode: { ready: true, path: 'OpenCode native runtime' }, storage: { isWritable: true, freeSpaceMb: 10000 } });
        if (cmd === 'register_account') { accounts.push({ instance: { uin: args.uin, httpPort: 3004, wsPort: 3005, webuiPort: 6101, autoStart: false }, login: { loggedIn: false, source: '' }, selected: false }); return ok({}); }
        if (cmd === 'batch_accounts') return ok(args.uins.map(uin => ({ uin, ok: true, detail: 'Completed' })));
        if (cmd === 'app_get_window_behavior') return ok({ closeToTray: true, minimizeToTray: false });
        if (cmd === 'plugin:app|version') return '0.5.0';
        if (cmd === 'get_napcat_version') return ok('4.18.28');
        return ok([]);
      },
    };
  });
  await page.reload();
  await page.getByRole('heading', { name: 'QQ 已成功连接' }).waitFor();
  await page.getByText('正在同步本地配置与联系人', { exact: true }).waitFor({ state: 'hidden' });
  await page.getByRole('heading', { name: '账号管理' }).waitFor();
  await page.screenshot({ path: 'B:/EazyQQ/output/ui/accounts-desktop.png' });
  await page.getByRole('textbox', { name: '添加 QQ 账号' }).fill('10003,10004');
  await page.getByRole('button', { name: '添加', exact: true }).click();
  await page.getByLabel('选择账号 10004').waitFor();
  await page.getByLabel('选择账号 10002').check();
  await page.getByRole('button', { name: '批量启动', exact: true }).click();
  await page.getByText('10002：完成', { exact: true }).waitFor();
  const calls = await page.evaluate(() => window.__fixtureCalls.filter(item => ['register_account', 'batch_accounts'].includes(item.cmd)));
  if (calls.filter(item => item.cmd === 'register_account').length !== 2) throw new Error('Batch registration did not cover both accounts');
  if (!calls.some(item => item.cmd === 'batch_accounts' && item.args.uins.join(',') === '10002')) throw new Error('Batch operation targeted the wrong accounts');
  await page.getByRole('listitem').filter({ hasText: '10002' }).getByRole('button', { name: '切换', exact: true }).click();
  await page.getByRole('region', { name: '目标账号扫码' }).waitFor();
  await page.getByText('QQ: 10001', { exact: true }).waitFor();
  await page.screenshot({ path: 'B:/EazyQQ/output/ui/target-account-qr.png', fullPage: true });
  await page.evaluate(() => { window.__fixtureAuthenticated = true; });
  await page.getByText('QQ: 10002', { exact: true }).waitFor();
  await page.getByRole('region', { name: '目标账号扫码' }).waitFor({ state: 'hidden' });
  await page.setViewportSize({ width: 900, height: 700 });
  await page.screenshot({ path: 'B:/EazyQQ/output/ui/accounts-compact.png' });
  return { registerCalls: 2, correctBatchTarget: true, targetQrPreservesCurrentAccount: true, confirmedTargetSelected: true, screenshots: 3 };
}
