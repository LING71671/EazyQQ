import React, { useState, useEffect } from 'react';
import { RefreshCw, AlertCircle, Sparkles, CheckCircle2, ExternalLink, Cpu, Download, Loader2, ArrowUpCircle } from 'lucide-react';
import type { AppUpdateInfo, NapCatUpdateInfo } from '@/api/contracts';
import { api } from '@/api/client';

export const AppUpdateCard: React.FC = () => {
  // EazyQQ App update
  const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  // NapCat Engine update
  const [napcatVersion, setNapcatVersion] = useState<string>('探测中...');
  const [napcatUpdate, setNapcatUpdate] = useState<NapCatUpdateInfo | null>(null);
  const [isCheckingNapCat, setIsCheckingNapCat] = useState(false);
  const [isUpgradingNapCat, setIsUpgradingNapCat] = useState(false);
  const [napcatNotice, setNapcatNotice] = useState<string | null>(null);
  const [napcatError, setNapcatError] = useState<string | null>(null);

  const fetchNapCatCurrentVersion = async () => {
    try {
      const res = await api.getNapCatVersion();
      if (res.success && res.data) {
        setNapcatVersion(res.data);
      }
    } catch {
      setNapcatVersion('2.7.3');
    }
  };

  useEffect(() => {
    fetchNapCatCurrentVersion();
  }, []);

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

  const handleCheckNapCatUpdate = async () => {
    setIsCheckingNapCat(true);
    setNapcatError(null);
    setNapcatNotice(null);
    try {
      const res = await api.checkNapCatUpdate();
      if (res.success && res.data) {
        setNapcatUpdate(res.data);
      } else {
        setNapcatError(res.error?.message || '检查 NapCat 官方版本失败');
      }
    } catch (e: any) {
      setNapcatError(e?.message || String(e));
    } finally {
      setIsCheckingNapCat(false);
    }
  };

  const handleUpgradeNapCat = async () => {
    setIsUpgradingNapCat(true);
    setNapcatNotice('正在下载 NapCat 官方更新包并覆盖部署，耗时约 10~30 秒，请稍候...');
    setNapcatError(null);
    try {
      const res = await api.upgradeNapCat(napcatUpdate?.downloadUrl);
      if (res.success) {
        setNapcatNotice(res.data || '升级完成，已自动重新唤醒协议服务！');
        fetchNapCatCurrentVersion();
        setNapcatUpdate((prev) => (prev ? { ...prev, hasUpdate: false } : null));
      } else {
        setNapcatError(res.error?.message || '升级执行失败');
        setNapcatNotice(null);
      }
    } catch (e: any) {
      setNapcatError(e?.message || String(e));
      setNapcatNotice(null);
    } finally {
      setIsUpgradingNapCat(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* 1. EazyQQ App update */}
      <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <h4 className="text-xs font-semibold text-slate-900">关于与版本更新</h4>
              <span className="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium bg-sky-50 text-sky-700 border border-sky-200/60">
                v0.3.0-beta
              </span>
            </div>
            <p className="text-[11px] text-slate-400 mt-0.5">
              EazyQQ 桌面客户端主程序版本维护
            </p>
          </div>
          <button
            onClick={handleCheckUpdate}
            disabled={isCheckingUpdate}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-slate-900 hover:bg-slate-800 text-white text-xs font-medium transition-colors shrink-0 disabled:opacity-50 shadow-xs cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isCheckingUpdate ? 'animate-spin' : ''}`} />
            <span>{isCheckingUpdate ? '检查中...' : '检查主程序更新'}</span>
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
                    <span>发现客户端新版本：{updateInfo.latestVersion}</span>
                  </>
                ) : (
                  <>
                    <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                    <span>当前客户端已是最新版本 (v{updateInfo.currentVersion})</span>
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

      {/* 2. NapCat Protocol Engine update */}
      <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <div className="w-5 h-5 rounded-md bg-indigo-50 text-indigo-600 flex items-center justify-center">
                <Cpu className="w-3.5 h-3.5" />
              </div>
              <h4 className="text-xs font-semibold text-slate-900">NapCat 协议核心组件</h4>
              <span className="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium bg-indigo-50 text-indigo-700 border border-indigo-200/60">
                v{napcatVersion}
              </span>
            </div>
            <p className="text-[11px] text-slate-400 mt-0.5">
              底层 QQ 协议通讯引擎，直接对接 NapNeko 官方 Release 通道
            </p>
          </div>
          <button
            onClick={handleCheckNapCatUpdate}
            disabled={isCheckingNapCat || isUpgradingNapCat}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-medium transition-colors shrink-0 disabled:opacity-50 shadow-xs cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isCheckingNapCat ? 'animate-spin' : ''}`} />
            <span>{isCheckingNapCat ? '嗅探中...' : '检查核心更新'}</span>
          </button>
        </div>

        {napcatNotice && (
          <div className="p-2.5 rounded-xl bg-indigo-50 border border-indigo-100 text-[11px] text-indigo-700 flex items-center gap-2">
            {isUpgradingNapCat ? <Loader2 className="w-4 h-4 animate-spin shrink-0" /> : <CheckCircle2 className="w-4 h-4 shrink-0" />}
            <span>{napcatNotice}</span>
          </div>
        )}

        {napcatError && (
          <div className="p-2.5 rounded-xl bg-red-50 border border-red-100 text-[11px] text-red-600 flex items-center gap-2">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{napcatError}</span>
          </div>
        )}

        {napcatUpdate && (
          <div
            className={`p-3 rounded-xl border text-xs space-y-2.5 ${
              napcatUpdate.hasUpdate
                ? 'bg-amber-50/60 border-amber-200/80 text-amber-900'
                : 'bg-slate-50 border-slate-200 text-slate-600'
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 font-semibold">
                {napcatUpdate.hasUpdate ? (
                  <>
                    <Sparkles className="w-4 h-4 text-amber-500" />
                    <span>发现 NapCat 核心新版本：v{napcatUpdate.latestVersion}</span>
                  </>
                ) : (
                  <>
                    <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                    <span>NapCat 协议核心已是最新版本 (v{napcatUpdate.currentVersion})</span>
                  </>
                )}
              </div>
              {napcatUpdate.hasUpdate && (
                <button
                  onClick={handleUpgradeNapCat}
                  disabled={isUpgradingNapCat}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-medium transition-colors shadow-2xs cursor-pointer disabled:opacity-50"
                >
                  {isUpgradingNapCat ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <ArrowUpCircle className="w-3.5 h-3.5" />}
                  <span>{isUpgradingNapCat ? '正在升级部署...' : '一键热升级核心'}</span>
                </button>
              )}
            </div>
            {napcatUpdate.releaseNotes && (
              <div className="text-[11px] opacity-80 whitespace-pre-wrap font-sans bg-white/60 p-2 rounded-lg border border-amber-100/50 max-h-36 overflow-y-auto">
                {napcatUpdate.releaseNotes}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
