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
  await page.addInitScript(() => {
    const callbacks = new Map(); let next = 100;
    const internal = window.__TAURI_INTERNALS__;
    internal.transformCallback = callback => { callbacks.set(++next,callback);return next; };
    const invoke = internal.invoke;
    internal.invoke = async (command,args={}) => {
      if (command === 'plugin:app|version') return '0.5.1';
      if (command === 'get_napcat_version') return {success:true,data:window.__coreInstalled ? '4.18.33' : '4.18.28'};
      if (command === 'check_napcat_update') return {success:true,data:{currentVersion:'4.18.28',latestVersion:'4.18.33',hasUpdate:true,status:'available',downloadUrl:'https://github.com/NapNeko/NapCatQQ/releases/download/v4.18.33/NapCat.Shell.zip',releaseNotes:'核心组件修复。'}};
      if (command === 'plugin:event|listen' && args.event === 'napcat-update-progress') { window.__coreCallback=callbacks.get(args.handler);return 124; }
      if (command === 'upgrade_napcat') return new Promise(resolve=>{window.__resolveCore=resolve;});
      if (command === 'check_app_update') return {success:true,data:{currentVersion:'0.5.1',latestVersion:'0.5.2',hasUpdate:true,status:'available',releaseName:'EazyQQ v0.5.2',releaseNotes:'修复协议启动，完善已校验安装流程。',htmlUrl:'https://github.com/LING71671/EazyQQ/releases',downloadUrl:'https://github.com/LING71671/EazyQQ/releases/download/v0.5.2/EazyQQ_0.5.2_x64-setup.exe',installerName:'EazyQQ_0.5.2_x64-setup.exe',downloadSize:26532105,checksumSha256:'a'.repeat(64)}};
      if (command === 'plugin:event|listen' && args.event === 'app-update-progress') { window.__updateCallback = callbacks.get(args.handler); return 123; }
      if (command === 'upgrade_app') return new Promise(resolve => { window.__resolveUpdate = resolve; });
      return invoke(command,args);
    };
  });
  await page.reload();
  await page.getByRole('heading',{name:'账号管理'}).waitFor();
  await page.getByRole('button',{name:'系统设置',exact:true}).click();
  await page.getByRole('button',{name:'检查更新',exact:true}).click();
  await page.getByText('可更新到 v0.5.2',{exact:true}).waitFor();
  await page.screenshot({path:'B:/EazyQQ/output/ui/update-available-desktop.png'});
  await page.setViewportSize({width:900,height:700});
  await page.getByRole('button',{name:'下载并安装',exact:true}).click();
  await page.evaluate(() => window.__updateCallback({payload:{phase:'downloading',downloadedBytes:13266052,totalBytes:26532105}}));
  await page.getByText('下载中 49%',{exact:true}).waitFor();
  if (!await page.getByRole('button',{name:'检查更新',exact:true}).isDisabled()) throw new Error('Check remained enabled during installation');
  await page.screenshot({path:'B:/EazyQQ/output/ui/update-progress-compact.png'});
  await page.evaluate(() => window.__resolveUpdate({success:false,error:{message:'安装包 SHA256 不匹配，已拒绝安装'}}));
  await page.getByRole('alert').filter({hasText:'安装包 SHA256 不匹配'}).waitFor();
  if (await page.getByRole('button',{name:'下载并安装',exact:true}).isDisabled()) throw new Error('Retry remained disabled');
  await page.screenshot({path:'B:/EazyQQ/output/ui/update-error-compact.png'});
  await page.getByRole('button',{name:'检查核心更新',exact:true}).click();
  await page.getByRole('button',{name:'停机更新核心',exact:true}).click();
  await page.evaluate(() => window.__coreCallback({payload:{phase:'downloading',downloadedBytes:50,totalBytes:100}}));
  await page.getByText('下载中 50%',{exact:true}).waitFor();
  if (!await page.getByRole('button',{name:'下载并安装',exact:true}).isDisabled()) throw new Error('App install remained enabled during core update');
  await page.screenshot({path:'B:/EazyQQ/output/ui/core-progress-compact.png'});
  await page.evaluate(()=>{window.__coreInstalled=true;window.__resolveCore({success:true,data:'协议更新完成，请启动所需账号。'});});
  await page.getByText('当前协议已是最新稳定版本 v4.18.33',{exact:true}).waitFor();
  await page.screenshot({path:'B:/EazyQQ/output/ui/core-complete-compact.png'});
  return {progress:true,error:true,retry:true,coreProgress:true,coreVersionRefresh:true,mutualExclusion:true};
}
