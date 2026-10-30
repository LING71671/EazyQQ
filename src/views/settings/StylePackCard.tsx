import { useEffect, useRef, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { api } from '@/api/client';
import type { StylePackStatus } from '@/api/contracts';

const buttonClass = 'rounded-lg border border-slate-300 px-3 py-2 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600';

export function StylePackCard() {
  const [status, setStatus] = useState<StylePackStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const pending = useRef(false);
  const mounted = useRef(true);
  const generation = useRef(0);

  useEffect(() => {
    let cancelled = false;
    mounted.current = true;
    const request = ++generation.current;
    api.manageStylePacks('list').then(response => {
      if (cancelled || request !== generation.current) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '无法读取风格包');
      setStatus(response.data);
    }).catch(error => { if (!cancelled && request === generation.current) setError(String(error instanceof Error ? error.message : error)); });
    return () => { cancelled = true; mounted.current = false; };
  }, []);

  async function run(operation: string, packJson?: string | (() => Promise<string>), packId?: string, destination?: string) {
    if (pending.current) return;
    pending.current = true;
    const request = ++generation.current;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const content = typeof packJson === 'function' ? await packJson() : packJson;
      if (!mounted.current) return;
      const response = await api.manageStylePacks(operation, content, packId, destination);
      if (!response.success || !response.data) throw new Error(response.error?.message || '风格包操作失败');
      if (mounted.current && request === generation.current) {
        setStatus(response.data);
        if (response.data.exportedPath) setNotice(`已导出：${response.data.exportedPath}`);
      }
    } catch (error) {
      if (mounted.current && request === generation.current) setError(error instanceof Error ? error.message : String(error));
    } finally {
      pending.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  async function importFile(file?: File) {
    if (!file) return;
    if (file.size > 65_536) { setError('风格包文件不能超过64KiB'); return; }
    await run('import', () => file.text().catch(() => { throw new Error('无法读取风格包文件，请重新选择UTF-8 JSON文件'); }));
  }

  async function exportPack() {
    if (!status?.activeId || pending.current) return;
    try {
      const destination = await save({ defaultPath: `${status.activeId}.qqstyle.json`, filters: [{ name: '风格包', extensions: ['json'] }] });
      if (destination && mounted.current) await run('export', undefined, status.activeId, destination);
    } catch { if (mounted.current) setError('无法选择导出位置，请重试'); }
  }

  const selected = status?.packs.find(pack => pack.id === status.activeId);
  return <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs" aria-labelledby="style-pack-heading" aria-busy={busy}>
    <div className="mb-4 flex items-start justify-between gap-4">
      <div>
        <h3 id="style-pack-heading" className="text-sm font-semibold text-slate-900">聊天风格包</h3>
        <p className="mt-1 text-xs text-slate-600">只对当前账号生效；导入后立即用于新回复。</p>
      </div>
      <button type="button" className={buttonClass} disabled={!selected || busy} onClick={() => run(status?.enabled ? 'disable' : 'enable')}>
        {status?.enabled ? '停用风格包' : '启用风格包'}
      </button>
    </div>
    <div className="flex flex-wrap items-end gap-3">
      <label className="min-w-48 flex-1 text-xs font-medium text-slate-700">当前风格包
        <select className="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 focus-visible:outline-2 focus-visible:outline-sky-600"
          value={status?.activeId || ''} disabled={busy || !status?.packs.length}
          onChange={event => run('select', undefined, event.target.value)}>
          {!selected && <option value="">未选择</option>}
          {status?.packs.map(pack => <option key={pack.id} value={pack.id}>{pack.name} · {pack.version}</option>)}
        </select>
      </label>
      <input ref={input} type="file" accept=".json,application/json" className="hidden" aria-label="风格包JSON文件"
        onChange={event => { const file = event.currentTarget.files?.[0]; event.currentTarget.value = ''; void importFile(file); }} />
      <button type="button" className={buttonClass} disabled={busy} onClick={() => input.current?.click()}>导入</button>
      <button type="button" className={buttonClass} disabled={!selected || busy} onClick={exportPack}>导出</button>
      <button type="button" className={buttonClass} disabled={!selected || busy} onClick={() => run('remove')}>移除当前包</button>
    </div>
    {selected && <p className="mt-3 text-xs text-slate-600" role="status">{status?.enabled ? '已启用' : '已停用'} · {selected.rules.sentenceLength === 'short' ? '偏好短句' : selected.rules.sentenceLength === 'detailed' ? '详细表达' : '适中句长'} · {selected.rules.qqFaceIds.length} 个偏好QQ表情</p>}
    {notice && <p role="status" className="mt-3 break-words text-xs text-slate-700">{notice}</p>}
    {error && <p role="alert" className="mt-3 break-words text-xs text-red-700">{error}</p>}
  </section>;
}
