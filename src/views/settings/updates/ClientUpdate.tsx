import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-shell';
import { useEffect, useState } from 'react';
import { AlertCircle, CheckCircle2, Download, ExternalLink, Loader2, RefreshCw } from 'lucide-react';
import { useAppUpdate } from '@/features/updates/useAppUpdate';
import '@/styles/updates.css';

export function ClientUpdate({ blocked, onBusy }: { blocked:boolean; onBusy:(busy:boolean)=>void }) {
  const [version, setVersion] = useState('');
  const update = useAppUpdate();
  useEffect(() => { onBusy(Boolean(update.busy)); }, [update.busy,onBusy]);
  useEffect(() => {
    let active = true;
    getVersion().then(value => { if (active) setVersion(value); }).catch(() => {});
    return () => { active = false; };
  }, []);
  const info = update.info;
  const downloading = update.busy === 'install';
  const percent = update.progress?.totalBytes ? Math.min(100, Math.floor(update.progress.downloadedBytes / update.progress.totalBytes * 100)) : null;
  const progressLabel = update.progress?.phase === 'verifying' ? '正在校验安装包 SHA256'
    : update.progress?.phase === 'ready' ? '校验完成，正在启动安装'
    : percent === null ? '正在获取安装包' : `下载中 ${percent}%`;
  const title = !info ? '' : info.status === 'no_release' ? '暂无稳定发布版本'
    : info.status === 'installer_missing' ? `v${info.latestVersion} 暂缺桌面安装包`
    : info.hasUpdate ? `可更新到 v${info.latestVersion}`
    : info.status === 'newer_local' ? `当前版本高于最新稳定版本 v${info.latestVersion}` : `当前已是最新稳定版本 v${info.currentVersion}`;
  return (
    <section className="p-5 rounded-2xl bg-white border border-slate-200/80 space-y-3" aria-labelledby="client-update-heading" aria-busy={Boolean(update.busy)}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <h4 id="client-update-heading" className="text-xs font-semibold text-slate-900">主程序更新</h4>
          <span className="text-xs font-mono text-sky-700">v{version || info?.currentVersion || '读取中'}</span>
        </div>
        <button type="button" onClick={update.check} disabled={blocked || Boolean(update.busy)} className="inline-flex items-center gap-1.5 px-3 py-2 rounded-lg bg-slate-900 text-white text-xs hover:bg-slate-800 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600">
          <RefreshCw aria-hidden="true" className={`w-3.5 h-3.5 ${update.busy === 'check' ? 'animate-spin' : ''}`} />
          {update.busy === 'check' ? '检查中…' : '检查更新'}
        </button>
      </div>
      {update.error && <p role="alert" className="flex items-start gap-2 text-xs text-red-700 break-words"><AlertCircle aria-hidden="true" className="w-4 h-4 shrink-0" />{update.error}</p>}
      <div role="status" aria-live="polite" className="space-y-2 text-xs text-slate-700">
        {downloading && <><p className="flex items-center gap-2"><Loader2 aria-hidden="true" className="w-4 h-4 animate-spin" />{progressLabel}</p><progress aria-label="安装包下载进度" value={percent ?? undefined} max={100} className="app-update-progress w-full h-2" /></>}
        {update.notice && <p className="flex items-start gap-2"><CheckCircle2 aria-hidden="true" className="w-4 h-4 shrink-0 text-emerald-700" />{update.notice}</p>}
      </div>
      {info && <div className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-xs font-medium text-slate-900">{title}</p>
          {info.hasUpdate && info.downloadUrl && info.checksumSha256 && <button type="button" onClick={update.install} disabled={blocked || Boolean(update.busy)} className="inline-flex items-center gap-1.5 px-3 py-2 bg-sky-600 hover:bg-sky-700 text-white rounded-lg text-xs disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600"><Download aria-hidden="true" className="w-3.5 h-3.5" />{downloading ? '正在更新…' : '下载并安装'}</button>}
        </div>
        {info.hasUpdate && info.downloadUrl && <p className="text-xs text-slate-600">安装包 {info.downloadSize ? `${(info.downloadSize / 1024 / 1024).toFixed(1)} MiB` : ''} · SHA256 校验。安装时 EazyQQ 自动退出，完成后重新打开；QQ 保持运行。</p>}
        {info.releaseNotes && <details className="text-xs text-slate-700"><summary className="cursor-pointer py-1">查看版本说明</summary><div className="mt-2 max-h-48 overflow-y-auto whitespace-pre-wrap break-words">{info.releaseNotes}</div></details>}
        <button type="button" onClick={() => { void open(info.htmlUrl || 'https://github.com/LING71671/EazyQQ/releases').catch(() => {}); }} className="inline-flex items-center gap-1.5 text-xs text-sky-700 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2"><ExternalLink aria-hidden="true" className="w-3.5 h-3.5" />打开发布页</button>
      </div>}
    </section>
  );
}
