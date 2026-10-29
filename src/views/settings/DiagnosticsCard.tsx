import React, { useRef, useState } from 'react';
import {
  X,
  Cpu,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  HelpCircle,
  ExternalLink,
  FileDown,
} from 'lucide-react';
import type { DependencyHealthReport } from '@/api/contracts';
import { api } from '@/api/client';
import { useChainStatus } from '@/features/health/useChainStatus';
import { describeRepair } from '@/features/health/repairResult';

export type ChainLink = {
  link: string;
  label: string;
  impact: string;
  health: 'ok' | 'unknown' | 'failed';
  detail: string;
  recommendedAction?: string;
};

interface DiagnosticsCardProps {
  health: DependencyHealthReport;
  onCheckHealth: () => void;
  onExportDiagnostics: () => void;
  diagnosticsPath?: string;
}

export const DiagnosticsCard: React.FC<DiagnosticsCardProps> = ({
  health,
  onCheckHealth,
  onExportDiagnostics,
  diagnosticsPath,
}) => {
  const { links: chainLinks, isLoading: chainLoading, error: chainError, refresh: loadChain } = useChainStatus();
  const chainFirstBreak = chainLinks.find(link => link.health === 'failed');
  const [isRepairing, setIsRepairing] = useState(false);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [actionFailed, setActionFailed] = useState(false);
  const repairing = useRef(false);

  const handleRepair = async () => {
    if (repairing.current) return;
    repairing.current = true;
    setIsRepairing(true);
    setActionFailed(false);
    setActionNotice('正在诊断并按故障恢复…');
    try {
      const res = await api.repairChain();
      if (!res.success || !res.data) throw new Error(res.error?.message || '未收到恢复结果');
      setActionNotice(describeRepair(res.data));
      setActionFailed(res.data.attempted && !res.data.ok);
      void loadChain();
      onCheckHealth();
    } catch (e) {
      setActionFailed(true);
      setActionNotice(`诊断/恢复失败：${e instanceof Error ? e.message : String(e)}。请重新检测后重试。`);
    } finally {
      repairing.current = false;
      setIsRepairing(false);
    }
  };

  const handleOpenQqDownload = () => {
    window.open('https://im.qq.com/pcqq/index.shtml', '_blank');
  };


  return (
    <div className="space-y-6">
      {/* Pre-flight Dependency Health Card */}
      <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
        <div className="flex items-center justify-between pb-2 border-b border-slate-100">
          <span className="text-sm font-semibold text-slate-900 flex items-center gap-2">
            <Cpu className="w-4 h-4 text-sky-600" />
            <span>前置运行环境状态</span>
          </span>
          <button
            onClick={onCheckHealth}
            className="flex items-center gap-1 text-xs text-sky-600 hover:text-sky-700 transition-colors cursor-pointer"
          >
            <RefreshCw className="w-3 h-3" />
            <span>重新自检</span>
          </button>
        </div>

        <div className="grid grid-cols-2 gap-3 text-xs">
          {/* NTQQ */}
          <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
            <div>
              <span className="font-semibold block text-slate-800">NTQQ 客户端</span>
              <span className="text-slate-400 text-[11px] truncate max-w-[180px] block">
                {health.qqNt?.path || '默认路径已捕获'}
              </span>
              {!health.qqNt?.ready && (
                <button
                  onClick={handleOpenQqDownload}
                  className="mt-1.5 inline-flex items-center gap-1 text-[11px] text-sky-600 hover:text-sky-700 font-medium hover:underline cursor-pointer"
                >
                  <span>前往官网下载安装</span>
                  <ExternalLink className="w-3 h-3" />
                </button>
              )}
            </div>
            {health.qqNt?.ready ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 text-amber-500 shrink-0" />
            )}
          </div>

          {/* OpenCode */}
          <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
            <div>
              <span className="font-semibold block text-slate-800">本地 OpenCode</span>
              <span className="text-slate-400 text-[11px] block">
                {health.openCode?.ready
                  ? '运行环境可用，推理状态见下方链路'
                  : health.openCode?.path?.includes('未配置')
                  ? '已检测到 CLI (需配置 API 凭证)'
                  : '未运行本地服务'}
              </span>

            </div>
            {health.openCode?.ready ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 text-amber-500 shrink-0" />
            )}
          </div>

          {/* Storage */}
          <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
            <div>
              <span className="font-semibold block text-slate-800">本地存储空间</span>
              <span className="text-slate-400 text-[11px]">
                {health.storage?.isWritable ? '可正常读写 (WAL已启用)' : '只读不可写'}
                {typeof health.storage?.freeSpaceMb === 'number' && health.storage.freeSpaceMb > 0
                  ? ` · 剩余 ${(health.storage.freeSpaceMb / 1024).toFixed(1)} GB`
                  : ''}
              </span>
            </div>
            {health.storage?.isWritable ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 text-red-500 shrink-0" />
            )}
          </div>

          {/* Overall verdict */}
          <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
            <div>
              <span className="font-semibold block text-slate-800">整体就绪状态</span>
              <span className="text-slate-400 text-[11px]">
                {health.isAllReady ? '全部前置条件已满足' : '存在未满足的运行条件，请查看对应说明'}
              </span>
            </div>
            {health.isAllReady ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 text-amber-500 shrink-0" />
            )}
          </div>
        </div>
      </div>

      {/* End-to-end chain status */}
      <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
        <div className="flex items-center justify-between pb-2 border-b border-slate-100">
          <div>
            <h4 className="text-xs font-semibold text-slate-900">全链路状态检测与一键修复</h4>
            <p className="text-[11px] text-slate-400">
              逐项检查 8 个环节；按故障原因恢复，模型与 Key 在配置区管理。
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleRepair}
              disabled={isRepairing}
              className="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 border border-sky-200 text-xs font-medium disabled:opacity-50 transition-colors shrink-0 cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRepairing ? 'animate-spin' : ''}`} />
              <span>{isRepairing ? '诊断/恢复中…' : '检查并按故障恢复'}</span>
            </button>
            <button
              onClick={loadChain}
              disabled={chainLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 text-xs text-slate-700 hover:bg-slate-100 disabled:opacity-50 transition-colors shrink-0 cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${chainLoading ? 'animate-spin' : ''}`} />
              <span>{chainLoading ? '检测中…' : '重新检测'}</span>
            </button>
          </div>
        </div>

        {actionNotice && (
          <div role={actionFailed ? 'alert' : 'status'} className={`p-3 rounded-xl border text-xs flex items-start justify-between gap-2 ${actionFailed ? 'bg-red-50 border-red-200 text-red-800' : 'bg-sky-50 border-sky-200 text-sky-800'}`}>
            <span>{actionNotice}</span>
            <button onClick={() => setActionNotice(null)} aria-label="关闭操作提示" className="text-sky-500 hover:text-sky-700 ml-2 cursor-pointer"><X className="h-4 w-4" /></button>
          </div>
        )}

        {chainFirstBreak ? (
          <div className="p-3 rounded-xl bg-red-50 border border-red-100 text-[11px] leading-relaxed flex items-center justify-between gap-3">
            <div>
              <span className="font-semibold text-red-900 block">
                首要阻断链路：{chainFirstBreak.label}
              </span>
              <span className="text-red-800 block mt-0.5">{chainFirstBreak.detail}</span>
              <span className="text-red-700 block mt-1">影响：{chainFirstBreak.impact}</span>
              {chainFirstBreak.recommendedAction && <span className="block mt-1 text-red-800">建议：{chainFirstBreak.recommendedAction}</span>}
            </div>
            {chainFirstBreak.label.includes('NapCat') || chainFirstBreak.label.includes('OneBot') ? (
              <button
                onClick={handleRepair}
                disabled={isRepairing}
                className="px-3 py-1.5 rounded-lg bg-red-600 hover:bg-red-700 text-white font-medium text-xs shrink-0 shadow-xs transition-colors cursor-pointer"
              >
                按故障恢复
              </button>
            ) : null}
          </div>
        ) : chainLinks.length > 0 ? (
          <div className={`p-3 rounded-xl border text-[11px] ${chainLinks.some(link => link.health === 'unknown') ? 'bg-slate-50 border-slate-200 text-slate-700' : 'bg-emerald-50 border-emerald-100 text-emerald-900'}`}>
            {chainLinks.some(link => link.health === 'unknown') ? '未发现已确认的故障，部分环节尚待验证。' : '全链路已验证，未发现断点。'}
          </div>
        ) : null}

        {chainError && <p role="alert" className="text-sm text-rose-700">{chainError}</p>}
        <div className="space-y-1.5">
          {chainLinks.map((link) => (
            <div
              key={link.link}
              data-chain-link={link.link}
              data-health={link.health}
              className="flex items-center justify-between py-2 border-b border-slate-50 last:border-0 gap-3"
            >
              <div className="flex items-start gap-2.5 min-w-0 flex-1">
                {link.health === 'ok' ? (
                  <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 mt-0.5 shrink-0" />
                ) : link.health === 'failed' ? (
                  <AlertCircle className="w-3.5 h-3.5 text-red-500 mt-0.5 shrink-0" />
                ) : (
                  <HelpCircle className="w-3.5 h-3.5 text-slate-300 mt-0.5 shrink-0" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2">
                    <span className="text-xs font-medium text-slate-800 shrink-0">{link.label}</span>
                    <span className="text-[11px] text-slate-500 truncate">{link.detail}</span>
                  </div>
                  {link.health === 'failed' && (
                    <span className="text-[10px] text-red-600 block mt-0.5">
                      影响：{link.impact}
                    </span>
                  )}
                  {link.recommendedAction && <span className="block mt-1 break-words text-xs text-slate-600">建议：{link.recommendedAction}</span>}
                </div>
              </div>

              {/* 1-Click Action Buttons for Links */}
              {link.health !== 'ok' && (
                <div className="shrink-0">
                  {(link.link.includes('napcat') || link.link.includes('one_bot')) && (
                    <button
                      onClick={handleRepair}
                      disabled={isRepairing}
                      className="px-2.5 py-1 text-[11px] rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 border border-sky-200 font-medium transition-colors cursor-pointer"
                    >
                      按故障恢复
                    </button>
                  )}

                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Diagnostics Export */}
      <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h4 className="text-xs font-semibold text-slate-900">一键导出诊断日志包</h4>
            <p className="text-[11px] text-slate-400">
              打包经过安全脱敏的本地运行日志、配置快照与崩溃报告为 ZIP，用于提交开发者查阅定位
            </p>
          </div>
          <button
            onClick={onExportDiagnostics}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 text-xs text-slate-700 hover:bg-slate-100 transition-colors shrink-0 cursor-pointer"
          >
            <FileDown className="w-3.5 h-3.5" />
            <span>导出诊断包</span>
          </button>
        </div>
        {diagnosticsPath && (
          <div className="p-2.5 rounded-lg bg-emerald-50 border border-emerald-100 text-[11px] text-emerald-900 font-mono break-all">
            已生成: {diagnosticsPath}
          </div>
        )}
      </div>
    </div>
  );
};
