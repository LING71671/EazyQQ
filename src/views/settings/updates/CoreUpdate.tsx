import { useEffect } from 'react';
import { AlertCircle, Download, Loader2, RefreshCw } from 'lucide-react';
import { useCoreUpdate } from '@/features/updates/useCoreUpdate';
import '@/styles/updates.css';

export function CoreUpdate({ blocked, onBusy }: { blocked:boolean; onBusy:(busy:boolean)=>void }) {
  const core = useCoreUpdate();
  useEffect(() => { onBusy(Boolean(core.busy)); }, [core.busy,onBusy]);
  const info = core.info;
  const percent = core.progress?.totalBytes ? Math.min(100, Math.floor(core.progress.downloadedBytes / core.progress.totalBytes * 100)) : null;
  const phase = core.progress?.phase;
  const progressText = phase === 'verifying' ? '正在校验协议包 SHA256' : phase === 'installing' ? '正在暂存与部署，保留配置和备份'
    : phase === 'ready' ? '协议更新完成' : phase === 'downloading' ? `下载中 ${percent ?? 0}%` : '正在确认更新与停机状态';
  const title = !info ? '' : info.status === 'asset_missing' ? '官方发布缺少 Shell 协议包'
    : info.status === 'version_unknown' ? `本机版本待确认，可安装官方 v${info.latestVersion}`
    : info.hasUpdate ? `可更新到 v${info.latestVersion}` : info.status === 'newer_local' ? `本机版本高于最新稳定版本 v${info.latestVersion}` : `当前协议已是最新稳定版本 v${info.currentVersion}`;
  return <section aria-labelledby="core-update-heading" aria-busy={Boolean(core.busy)} className="p-5 rounded-2xl bg-white border border-slate-200/80 space-y-3">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex items-center gap-2"><h4 id="core-update-heading" className="text-xs font-semibold text-slate-900">NapCat 核心组件</h4><span className="text-xs font-mono text-sky-700">{core.version === 'unknown' ? '版本待确认' : `v${core.version}`}</span></div>
      <button type="button" disabled={blocked || Boolean(core.busy)} onClick={core.check} className="inline-flex items-center gap-1.5 px-3 py-2 bg-slate-900 hover:bg-slate-800 text-white rounded-lg text-xs disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600"><RefreshCw aria-hidden="true" className={`w-3.5 h-3.5 ${core.busy === 'check' ? 'animate-spin' : ''}`} />{core.busy === 'check' ? '检查中…' : '检查核心更新'}</button>
    </div>
    <p className="text-xs text-slate-600">官方 Shell 包 · SHA256 校验 · 保留配置与回滚备份。升级前需停止相关协议账号。</p>
    {core.error && <p role="alert" className="flex items-start gap-2 text-xs text-red-700 break-words"><AlertCircle aria-hidden="true" className="w-4 h-4 shrink-0" />{core.error}</p>}
    <div role="status" aria-live="polite" className="space-y-2 text-xs text-slate-700">
      {core.busy === 'install' && <><p className="flex items-center gap-2"><Loader2 aria-hidden="true" className="w-4 h-4 animate-spin" />{progressText}</p><progress aria-label="核心组件下载进度" value={percent ?? undefined} max={100} className="app-update-progress w-full h-2" /></>}
      {core.notice && <p>{core.notice}</p>}
    </div>
    {info && <div className="space-y-3"><div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs font-medium text-slate-900">{title}</p>
      {info.hasUpdate && info.downloadUrl && <button type="button" disabled={blocked || Boolean(core.busy)} onClick={core.install} className="inline-flex items-center gap-1.5 px-3 py-2 bg-sky-600 hover:bg-sky-700 text-white rounded-lg text-xs disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600"><Download aria-hidden="true" className="w-3.5 h-3.5" />{core.busy === 'install' ? '更新中…' : '停机更新核心'}</button>}
    </div>{info.releaseNotes && <details className="text-xs text-slate-700"><summary className="cursor-pointer py-1">查看核心版本说明</summary><div className="mt-2 max-h-48 overflow-y-auto whitespace-pre-wrap break-words">{info.releaseNotes}</div></details>}</div>}
  </section>;
}
