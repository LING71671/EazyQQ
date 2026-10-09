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
        if (cmd === 'get_protocol_status') return ok(window.__restored ? {isConnected:true,loginStatus:'waiting_scan',qrcodeBase64:'https://example.test/recovered-qr'} : {isConnected:false,loginStatus:'unlogged',qrcodeError:'协议文件缺失：napcat.mjs。请点击恢复协议，使用新版资源重建运行目录。'});
        if (cmd === 'restart_napcat') { window.__restored=true; return ok({attempted:true,ok:true,detail:'Private protocol fixture restored'}); }
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
  await page.getByText('二维码服务不可用',{exact:true}).waitFor();
  await page.getByRole('alert').filter({hasText:'napcat.mjs'}).waitFor();
  if(await page.getByText('服务加载中',{exact:true}).count()) throw new Error('Offline state remained loading');
  await page.screenshot({path:'B:/EazyQQ/output/ui/login-unavailable.png'});
  await page.getByRole('button',{name:'恢复协议',exact:true}).click();
  await page.getByAltText('Login QR Code').waitFor();
  if(await page.getByRole('alert').count()) throw new Error('Recovered QR retained an error');
  await page.screenshot({path:'B:/EazyQQ/output/ui/login-recovered.png'});
  const calls=await page.evaluate(()=>window.__fixtureCalls);
  if(calls.some(call=>call.cmd==='quick_login')) throw new Error('Status polling caused login');
  if(calls.filter(call=>call.cmd==='restart_napcat').length!==1) throw new Error('Recovery was not explicit');
  return {offlineError:true,explicitRecovery:true,qrVisible:true,pollingReadOnly:true,realQQTouched:false};
}
