import { useCallback, useEffect, useRef, useState } from 'react';
import { qrImage } from '@/features/accounts/qrImage';
import { api } from '@/api/client';
import type { AccountReport } from '@/api/contracts';

export function AccountManager({ onSelect, switching }: { onSelect?: (uin: string) => void; switching?: string | null }) {
  const [accounts, setAccounts] = useState<AccountReport[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [qr, setQr] = useState<{ uin: string; image: string } | null>(null);
  const alive = useRef(true);
  const activeOperation = useRef(false);
  const fetching = useRef(false);
  const epoch = useRef(0);
  const refresh = useCallback(async () => {
    if (fetching.current) return;
    fetching.current = true;
    const current = epoch.current;
    try {
      const response = await api.listAccounts();
      if (alive.current && current === epoch.current && response.success && response.data) {
        setAccounts(response.data);
        const valid = new Set(response.data.map(item => item.instance.uin));
        setSelected(previous => previous.filter(uin => valid.has(uin)));
      }
    } catch (e) { if (alive.current && current === epoch.current) setNotice(String(e)); }
    finally { fetching.current = false; }
  }, []);
  useEffect(() => {
    alive.current = true;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => { await refresh(); if (!stopped) timer = setTimeout(poll, 10000); };
    void poll();
    return () => { stopped = true; alive.current = false; epoch.current += 1; clearTimeout(timer); };
  }, [refresh]);

  const run = async (operation: () => Promise<void>) => {
    if (activeOperation.current) return;
    activeOperation.current = true;
    epoch.current += 1;
    setBusy(true); setNotice('');
    try { await operation(); await refresh(); }
    catch (e) { if (alive.current) setNotice(e instanceof Error ? e.message : String(e)); }
    finally { activeOperation.current = false; if (alive.current) setBusy(false); }
  };
  const batch = (operation: 'start' | 'stop' | 'login') => run(async () => {
    const result = await api.batchAccounts(operation, selected);
    if (!result.success) throw new Error(result.error?.message || '批量操作失败');
    if (alive.current) setNotice(result.data?.map(item => `${item.uin}：${item.ok ? '完成' : item.detail}`).join('；') || '操作完成');
  });
  const showQr = (uin: string, refreshCode = false) => run(async () => {
    const result = await api.accountQrCode(uin, refreshCode);
    if (!result.success || !result.data) throw new Error(result.error?.message || '二维码未就绪');
    if (result.data.loggedIn) { if (alive.current) { setQr(null); setNotice(`${uin} 已登录，可切换至此账号`); } return; }
    const raw = result.data.qrcodeBase64;
    if (!raw) throw new Error('未收到二维码');
    const image = await qrImage(raw);
    if (alive.current) setQr({ uin, image });
  });

  return <section aria-label="多账号管理" className="mt-6 w-full max-w-lg rounded-2xl border border-slate-200 bg-white p-5 text-left dark:border-slate-700 dark:bg-slate-900">
    <div className="flex items-center justify-between gap-3">
      <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">账号管理</h3>
      <button type="button" onClick={() => void refresh()} disabled={busy} className="rounded-lg px-3 py-2 text-sm text-sky-700 hover:bg-sky-50 focus-visible:outline-2 focus-visible:outline-sky-600 disabled:opacity-50">刷新</button>
    </div>
    <form className="mt-3 flex gap-2" onSubmit={event => { event.preventDefault(); void run(async () => {
      const uins = [...new Set(input.split(/[,，\s]+/).filter(Boolean))];
      if (!uins.length || uins.some(uin => !/^[1-9]\d{4,19}$/.test(uin))) throw new Error('填写有效 QQ 号，多个账号用逗号分隔');
      for (const uin of uins) { const result = await api.registerAccount(uin); if (!result.success) throw new Error(result.error?.message || `${uin} 登记失败`); }
      setInput('');
    }); }}>
      <input aria-label="添加 QQ 账号" value={input} onChange={event => setInput(event.target.value)} placeholder="QQ 号，支持逗号分隔" className="min-w-0 flex-1 rounded-lg border border-slate-300 bg-transparent px-3 py-2 text-sm text-slate-900 placeholder:text-slate-500 focus-visible:outline-2 focus-visible:outline-sky-600 dark:text-slate-100" />
      <button disabled={busy || !input.trim()} className="rounded-lg bg-sky-600 px-3 py-2 text-sm text-white hover:bg-sky-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:opacity-50">添加</button>
    </form>
    {accounts.length > 0 && <>
      <div className="mt-4 flex flex-wrap items-center gap-2 border-b border-slate-200 pb-3">
        <label className="flex items-center gap-2 text-sm text-slate-700 dark:text-slate-300"><input type="checkbox" checked={selected.length === accounts.length} onChange={event => setSelected(event.target.checked ? accounts.map(item => item.instance.uin) : [])} />全选</label>
        {(['start', 'login', 'stop'] as const).map((operation, index) => <button key={operation} type="button" disabled={busy || !selected.length || !!switching} onClick={() => void batch(operation)} className="rounded-lg border border-slate-300 px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-sky-600 disabled:opacity-50 dark:text-slate-200">{['批量启动', '批量登录', '批量停止'][index]}</button>)}
      </div>
      <ul className="divide-y divide-slate-100 dark:divide-slate-800">
        {accounts.map(({ instance, login, selected: current }) => <li key={instance.uin} className="flex flex-wrap items-center gap-3 py-3">
          <input aria-label={`选择账号 ${instance.uin}`} type="checkbox" checked={selected.includes(instance.uin)} onChange={event => setSelected(previous => event.target.checked ? [...previous, instance.uin] : previous.filter(id => id !== instance.uin))} />
          <div className="min-w-0 flex-1"><span className="block truncate text-sm font-medium text-slate-900 dark:text-slate-100">{login.nickname || instance.nickname || instance.uin}</span><span className="text-xs text-slate-600 dark:text-slate-400">{instance.uin} · {login.loggedIn ? '在线' : login.source === 'webui' ? '待登录' : '未连接'}{current ? ' · 当前账号' : ''}</span></div>
          <label className="flex items-center gap-1 text-xs text-slate-600 dark:text-slate-400"><input type="checkbox" aria-label={`${instance.uin} 自动启动`} checked={instance.autoStart} disabled={busy} onChange={event => void run(async () => { const res = await api.configureAccount(instance.uin, event.target.checked); if (!res.success) throw new Error(res.error?.message); })} />自启</label>
          <button type="button" disabled={busy || !!switching} onClick={() => void showQr(instance.uin)} className="rounded-lg px-2 py-2 text-sm text-sky-700 hover:bg-sky-50 focus-visible:outline-2 focus-visible:outline-sky-600 disabled:opacity-50">扫码</button>
          <button type="button" disabled={busy || !!switching || current} onClick={() => onSelect?.(instance.uin)} className="rounded-lg px-2 py-2 text-sm text-sky-700 hover:bg-sky-50 focus-visible:outline-2 focus-visible:outline-sky-600 disabled:opacity-50">{switching === instance.uin ? '确认登录中…' : '切换'}</button>
        </li>)}
      </ul>
    </>}
    {qr && <div className="mt-4 flex flex-col items-center border-t border-slate-200 pt-4"><p className="text-sm text-slate-700 dark:text-slate-300">{qr.uin} 扫码登录</p><img src={qr.image} alt={`QQ ${qr.uin} 登录二维码`} width={220} height={220} /><div className="flex gap-2"><button type="button" disabled={busy} onClick={() => void showQr(qr.uin, true)} className="rounded-lg px-3 py-2 text-sm text-sky-700 focus-visible:outline-2 focus-visible:outline-sky-600">刷新二维码</button><button type="button" onClick={() => setQr(null)} className="rounded-lg px-3 py-2 text-sm text-slate-600 focus-visible:outline-2 focus-visible:outline-sky-600">关闭</button></div></div>}
    <p role="status" aria-live="polite" className="mt-3 break-words text-sm text-slate-700 dark:text-slate-300">{busy ? '正在处理账号操作…' : notice}</p>
  </section>;
}
