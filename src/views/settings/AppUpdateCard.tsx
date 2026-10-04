import React, { useState } from 'react';
import { RefreshCw, AlertCircle, Sparkles, CheckCircle2, ExternalLink } from 'lucide-react';
import type { AppUpdateInfo } from '@/api/contracts';
import { api } from '@/api/client';

export const AppUpdateCard: React.FC = () => {
  const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  const handleCheckUpdate = async () => {
    setIsCheckingUpdate(true);
    setUpdateError(null);
    try {
      const res = await api.checkAppUpdate();
      if (res.success && res.data) {
        setUpdateInfo(res.data);
      } else {
        setUpdateError(res.error?.message || '检查更新失败');
      }
    } catch (e: any) {
      setUpdateError(e?.message || String(e));
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  return (
    <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-2">
            <h4 className="text-xs font-semibold text-slate-900">关于与版本更新</h4>
            <span className="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium bg-sky-50 text-sky-700 border border-sky-200/60">
              v0.1.0-beta
            </span>
          </div>
          <p className="text-[11px] text-slate-400 mt-0.5">
            支持连接 GitHub Releases 官方通道自动检索版本更新
          </p>
        </div>
        <button
          onClick={handleCheckUpdate}
          disabled={isCheckingUpdate}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-slate-900 hover:bg-slate-800 text-white text-xs font-medium transition-colors shrink-0 disabled:opacity-50 shadow-xs cursor-pointer"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${isCheckingUpdate ? 'animate-spin' : ''}`} />
          <span>{isCheckingUpdate ? '检查中...' : '检查更新'}</span>
        </button>
      </div>

      {updateError && (
        <div className="p-2.5 rounded-xl bg-red-50 border border-red-100 text-[11px] text-red-600 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{updateError}</span>
        </div>
      )}

      {updateInfo && (
        <div
          className={`p-3 rounded-xl border text-xs space-y-2 ${
            updateInfo.hasUpdate
              ? 'bg-amber-50/60 border-amber-200/80 text-amber-900'
              : 'bg-slate-50 border-slate-200 text-slate-600'
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 font-semibold">
              {updateInfo.hasUpdate ? (
                <>
                  <Sparkles className="w-4 h-4 text-amber-500" />
                  <span>发现新版本：{updateInfo.latestVersion}</span>
                </>
              ) : (
                <>
                  <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                  <span>当前已是最新版本 (v{updateInfo.currentVersion})</span>
                </>
              )}
            </div>
            {updateInfo.hasUpdate && (
              <a
                href={updateInfo.downloadUrl || updateInfo.htmlUrl}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 px-3 py-1 bg-amber-600 hover:bg-amber-700 text-white rounded-lg text-xs font-medium transition-colors shadow-2xs"
              >
                <span>下载最新 Release</span>
                <ExternalLink className="w-3 h-3" />
              </a>
            )}
          </div>
          {updateInfo.releaseNotes && (
            <div className="text-[11px] opacity-80 whitespace-pre-wrap font-sans bg-white/60 p-2 rounded-lg border border-amber-100/50">
              {updateInfo.releaseNotes}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
