import React, { useState, useEffect } from 'react';
import { usePresence } from '@/hooks/usePresence';
import {
  Activity,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  RefreshCw,
  HardDrive,
  Network,
  Cpu,
  Download,
  FolderOpen,
  X,
  Wrench,
  ExternalLink,
  Zap,
  ShieldCheck,
  Loader2,
} from 'lucide-react';
import { api } from '@/api/client';
import type { DependencyHealthReport } from '@/api/contracts';

interface ChainLink {
  link: string;
  label: string;
  impact: string;
  health: 'ok' | 'unknown' | 'failed';
  detail: string;
  last_ok_secs_ago: number | null;
  last_error_secs_ago: number | null;
}

interface ChainHealthDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenSettings?: () => void;
}

export const ChainHealthDrawer: React.FC<ChainHealthDrawerProps> = ({
  isOpen,
  onClose,
  onOpenSettings,
}) => {
  const [links, setLinks] = useState<ChainLink[]>([]);
  const [firstBreak, setFirstBreak] = useState<{
    link: string;
    label: string;
    impact: string;
    detail: string;
  } | null>(null);
  const [hasFailure, setHasFailure] = useState(false);
  const [uptimeSecs, setUptimeSecs] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  // 1-Click Repair states
  const [isRestarting, setIsRestarting] = useState(false);
  const [repairNotice, setRepairNotice] = useState<string | null>(null);

  // Diagnostics Export
  const [isExporting, setIsExporting] = useState(false);
  const [exportedZipPath, setExportedZipPath] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  // AI Quick ping test
  const [isTestingAi, setIsTestingAi] = useState(false);
  const [aiTestResult, setAiTestResult] = useState<{ isSuccess: boolean; latencyMs: number } | null>(null);

  const fetchStatus = async () => {
    setIsLoading(true);
    try {
      const res = await api.getChainStatus();
      if (res.success && res.data) {
        setLinks(res.data.links as ChainLink[]);
        setFirstBreak(res.data.firstBreak);
        setHasFailure(res.data.hasFailure);
        setUptimeSecs(res.data.uptimeSecs);
      }
    } catch (e) {
      console.error('Failed to load chain status', e);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      fetchStatus();
      const interval = setInterval(fetchStatus, 5000);
      return () => clearInterval(interval);
    }
  }, [isOpen]);

  const handleRestartNapCat = async () => {
    setIsRestarting(true);
    setRepairNotice('正在检查故障类型…');
    try {
      const res = await api.repairChain();
      if (res.success) {
        setRepairNotice(res.data?.detail || '诊断完成');
        setTimeout(() => {
          fetchStatus();

        }, 4000);
      } else {
        setRepairNotice(`重启失败：${res.error?.message || '未知错误'}`);
      }
    } catch (e: any) {
      setRepairNotice(`重启异常：${e?.message || String(e)}`);
    } finally {
      setIsRestarting(false);
    }
  };

  const handleExportDiagnostics = async () => {
    setIsExporting(true);
    setExportError(null);
    try {
      const res = await api.exportDiagnosticsBundle();
      if (res.success && res.data?.zipFilePath) {
        setExportedZipPath(res.data.zipFilePath);
      } else {
        setExportError(res.error?.message || '导出诊断包失败');
      }
    } catch (e: any) {
      setExportError(e?.message || String(e));
    } finally {
      setIsExporting(false);
    }
  };

  const handleOpenZipFolder = async () => {
    if (!exportedZipPath) return;
    try {
      await api.openFolder(exportedZipPath);
    } catch (e) {
      console.error('Failed to open zip folder', e);
    }
  };

  const handleTestAi = async () => {
    setIsTestingAi(true);
    setAiTestResult(null);
    try {
      const cfgRes = await api.getConfig();
      const provider = cfgRes.data?.ai?.activeProvider || 'opencode';
      const model = cfgRes.data?.ai?.model;
      const res = await api.testAiConnection(provider, model);
      if (res.success && res.data) {
        setAiTestResult(res.data);
      } else {
        setAiTestResult({ isSuccess: false, latencyMs: 0 });
      }
    } catch {
      setAiTestResult({ isSuccess: false, latencyMs: 0 });
    } finally {
      setIsTestingAi(false);
    }
  };

  const mounted = usePresence(isOpen);
  if (!mounted) return null;

  const formatUptime = (secs: number) => {
    const mins = Math.floor(secs / 60);
    const hrs = Math.floor(mins / 60);
    if (hrs > 0) return `${hrs}小时${mins % 60}分`;
    if (mins > 0) return `${mins}分钟`;
    return `${secs}秒`;
  };

  return (
    <div data-open={isOpen} aria-hidden={!isOpen} inert={!isOpen} className="motion-overlay fixed inset-0 z-50 flex justify-end bg-slate-900/30">
      <div data-open={isOpen} className="motion-drawer w-full max-w-md h-full bg-white shadow-2xl border-l border-slate-200/80 flex flex-col overflow-hidden">
        {/* Drawer Header */}
        <div className="h-14 px-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50 shrink-0">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-xl bg-sky-100/70 text-sky-600 flex items-center justify-center">
              <Activity className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-slate-900 tracking-tight">链路健康体检 & 自愈</h2>
              <p className="text-[10px] text-slate-400">运行时间：{formatUptime(uptimeSecs)}</p>
            </div>
          </div>

          <div className="flex items-center gap-1">
            <button
              onClick={fetchStatus}
              disabled={isLoading}
              title="刷新检测"
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-700 hover:bg-slate-100 transition-colors disabled:opacity-50"
            >
              <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
            </button>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-700 hover:bg-slate-100 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Drawer Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-4 text-xs">
          {/* 1. Global Status Banner */}
          {hasFailure && firstBreak ? (
            <div className="p-4 rounded-2xl bg-amber-50/80 border border-amber-200/80 space-y-3">
              <div className="flex items-start gap-2.5">
                <AlertTriangle className="w-5 h-5 text-amber-600 shrink-0 mt-0.5" />
                <div className="min-w-0 flex-1">
                  <h3 className="font-bold text-amber-900 text-xs">
                    发现链路断点：{firstBreak.label}
                  </h3>
                  <p className="text-[11px] text-amber-700 leading-relaxed mt-1">
                    {firstBreak.detail || '服务未正常响应'}
                  </p>
                  <p className="text-[10px] text-amber-600/80 mt-0.5">
                    影响：{firstBreak.impact}
                  </p>
                </div>
              </div>

              {/* 1-Click Action for this break */}
              <div className="pt-2 border-t border-amber-200/60 flex items-center justify-between">
                <span className="text-[11px] font-medium text-amber-800">故障处理：</span>
                <button
                  onClick={handleRestartNapCat}
                  disabled={isRestarting}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-amber-600 hover:bg-amber-700 text-white font-medium shadow-xs transition-all active:scale-95 disabled:opacity-50 cursor-pointer"
                >
                  <Wrench className={`w-3.5 h-3.5 ${isRestarting ? 'animate-spin' : ''}`} />
                  <span>{isRestarting ? '正在自愈...' : '检查并修复'}</span>
                </button>
              </div>

              {repairNotice && (
                <div className="text-[11px] p-2 rounded-lg bg-amber-100/60 text-amber-900">
                  {repairNotice}
                </div>
              )}
            </div>
          ) : (
            <div className="p-3.5 rounded-2xl bg-emerald-50/80 border border-emerald-200/80 flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-emerald-100 text-emerald-600 flex items-center justify-center shrink-0">
                <CheckCircle2 className="w-4 h-4" />
              </div>
              <div>
                <h3 className="font-bold text-emerald-900 text-xs">{links.length > 0 && links.every(link => link.health === 'ok') ? '全链路已验证' : '等待完成链路检测'}</h3>
                <p className="text-[11px] text-emerald-700 mt-0.5">
                  查看各项检测结果；未就绪表示尚未验证。
                </p>
              </div>
            </div>
          )}

          {/* 2. Chain Link Detail Cards */}
          <div className="space-y-2">
            <h4 className="font-bold text-slate-800 text-xs tracking-tight flex items-center justify-between">
              <span>全链路 {links.length} 项状态</span>
              <span className="text-[10px] text-slate-400 font-normal">自动每 5 秒嗅探</span>
            </h4>

            <div className="space-y-2">
              {links.map((link) => {
                const isOk = link.health === 'ok';
                const isFailed = link.health === 'failed';

                return (
                  <div
                    key={link.link}
                    className={`p-3 rounded-xl border transition-all ${
                      isOk
                        ? 'bg-slate-50/60 border-slate-200/80'
                        : isFailed
                        ? 'bg-red-50/50 border-red-200'
                        : 'bg-amber-50/40 border-amber-200'
                    }`}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <div className="flex items-center gap-2">
                        <span
                          className={`w-2 h-2 rounded-full ${
                            isOk
                              ? 'bg-emerald-500 ring-2 ring-emerald-200'
                              : isFailed
                              ? 'bg-red-500 ring-2 ring-red-200'
                              : 'bg-amber-400'
                          }`}
                        />
                        <span className="font-semibold text-slate-800 text-xs">{link.label}</span>
                      </div>
                      <span
                        className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${
                          isOk
                            ? 'bg-emerald-100 text-emerald-700'
                            : isFailed
                            ? 'bg-red-100 text-red-700'
                            : 'bg-amber-100 text-amber-700'
                        }`}
                      >
                        {isOk ? '健康' : isFailed ? '异常' : '未就绪'}
                      </span>
                    </div>

                    <p className="text-[11px] text-slate-500 leading-relaxed pl-4">
                      {link.detail || (isOk ? '正常运行中' : '等待连接')}
                    </p>

                    {/* Specific Node Actions */}
                    {link.link === 'ai' && (
                      <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex items-center justify-between pl-4">
                        <span className="text-[10px] text-slate-400">大模型推理</span>
                        <div className="flex items-center gap-1.5">
                          <button
                            onClick={handleTestAi}
                            disabled={isTestingAi}
                            className="flex items-center gap-1 px-2 py-1 rounded-lg bg-white border border-slate-200 text-slate-700 hover:bg-slate-50 transition-colors text-[10px] font-medium cursor-pointer"
                          >
                            <Zap className={`w-3 h-3 text-amber-500 ${isTestingAi ? 'animate-spin' : ''}`} />
                            <span>{isTestingAi ? '测试中...' : '快速测速'}</span>
                          </button>
                          {onOpenSettings && (
                            <button
                              onClick={() => {
                                onClose();
                                onOpenSettings();
                              }}
                              className="px-2 py-1 rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 transition-colors text-[10px] font-medium cursor-pointer"
                            >
                              配置模型
                            </button>
                          )}
                        </div>
                      </div>
                    )}

                    {link.link === 'ai' && aiTestResult && (
                      <div className="mt-1.5 pl-4 text-[10px] flex items-center gap-1 text-slate-600">
                        {aiTestResult.isSuccess ? (
                          <span className="text-emerald-600 font-medium">
                            ✓ 连通正常，延迟 {aiTestResult.latencyMs}ms
                          </span>
                        ) : (
                          <span className="text-red-500 font-medium">✗ 接口测试未响应</span>
                        )}
                      </div>
                    )}

                    {link.link === 'napcat' && !isOk && (
                      <div className="mt-2 pl-4">
                        <button
                          onClick={handleRestartNapCat}
                          disabled={isRestarting}
                          className="px-2.5 py-1 rounded-lg bg-sky-600 hover:bg-sky-700 text-white text-[10px] font-medium transition-colors cursor-pointer"
                        >
                          唤醒 NapCat
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* 3. Manual QQ Check Guidance for Beginners */}
          <div className="p-3.5 rounded-xl bg-slate-50 border border-slate-200/80 space-y-2">
            <h4 className="font-semibold text-slate-800 text-xs flex items-center gap-1.5">
              <ShieldCheck className="w-3.5 h-3.5 text-sky-600" />
              <span>小白常见排障指南</span>
            </h4>
            <ul className="text-[11px] text-slate-500 space-y-1 list-disc list-inside leading-relaxed">
              <li>若提示找不到 QQ：请先启动电脑版腾讯 QQ 并保持后台运行。</li>
              <li>若登录二维码失效：点击登录页下方「刷新二维码」重新生成。</li>
              <li>若大模型无回复：可在设置中切换到 OpenCode 免费官方源。</li>
            </ul>
            <div className="pt-1">
              <a
                href="https://im.qq.com/pcqq/index.shtml"
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 text-[11px] text-sky-600 hover:text-sky-700 font-medium"
              >
                <span>腾讯 QQ 官方下载安装地址</span>
                <ExternalLink className="w-3 h-3" />
              </a>
            </div>
          </div>

          {/* 4. Diagnostics Export Button */}
          <div className="p-3.5 rounded-xl bg-white border border-slate-200/80 space-y-2.5">
            <div className="flex items-center justify-between">
              <div>
                <span className="font-semibold text-slate-800 block text-xs">一键导出脱敏诊断包</span>
                <span className="text-slate-400 text-[10px]">
                  遇到无法自主解决的问题？导出 zip 日志发送给技术协助
                </span>
              </div>
              <button
                onClick={handleExportDiagnostics}
                disabled={isExporting}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-slate-900 hover:bg-slate-800 text-white text-xs font-medium transition-all cursor-pointer active:scale-95 disabled:opacity-50"
              >
                {isExporting ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Download className="w-3.5 h-3.5" />
                )}
                <span>{isExporting ? '打包中…' : '导出日志'}</span>
              </button>
            </div>

            {exportedZipPath && (
              <div className="p-2.5 rounded-lg bg-emerald-50 border border-emerald-200 text-emerald-800 text-[11px] flex items-center justify-between">
                <span className="truncate pr-2">已保存至：{exportedZipPath}</span>
                <button
                  onClick={handleOpenZipFolder}
                  className="flex items-center gap-1 shrink-0 text-emerald-700 hover:text-emerald-900 font-semibold cursor-pointer"
                >
                  <FolderOpen className="w-3.5 h-3.5" />
                  <span>打开</span>
                </button>
              </div>
            )}

            {exportError && (
              <div className="p-2.5 rounded-lg bg-red-50 border border-red-200 text-red-700 text-[11px]">
                {exportError}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
