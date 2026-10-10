import React, { useState, useEffect } from 'react';
import {
  X,
  Cpu,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  HelpCircle,
  ExternalLink,
  Zap,
  FileDown,
} from 'lucide-react';
import type { DependencyHealthReport } from '@/api/contracts';
import { api } from '@/api/client';

export type ChainLink = {
  link: string;
  label: string;
  impact: string;
  health: 'ok' | 'unknown' | 'failed';
  detail: string;
};

interface DiagnosticsCardProps {
  health: DependencyHealthReport;
  onCheckHealth: () => void;
  onQuickSwitchOpenCode: () => void;
  onExportDiagnostics: () => void;
  diagnosticsPath?: string;
}

export const DiagnosticsCard: React.FC<DiagnosticsCardProps> = ({
  health,
  onCheckHealth,
  onQuickSwitchOpenCode,
  onExportDiagnostics,
  diagnosticsPath,
}) => {
  const [chainLinks, setChainLinks] = useState<ChainLink[]>([]);
  const [chainFirstBreak, setChainFirstBreak] = useState<{
    label: string;
    detail: string;
    impact: string;
  } | null>(null);
  const [chainLoading, setChainLoading] = useState(false);
  const [isRestartingNapcat, setIsRestartingNapcat] = useState(false);
  const [actionNotice, setActionNotice] = useState<string | null>(null);

  const loadChain = async () => {
    setChainLoading(true);
    try {
      const res = await api.getChainStatus();
      if (res.success && res.data) {
        setChainLinks(res.data.links as ChainLink[]);
        setChainFirstBreak(res.data.firstBreak ?? null);
      }
    } catch (e) {
      console.error('Failed to load chain status', e);
    } finally {
      setChainLoading(false);
    }
  };

  const handleRestartProtocol = async () => {
    setIsRestartingNapcat(true);
    setActionNotice('正在唤醒/重启 NapCat 协议端...');
    try {
      const res = await api.restartNapCat();
      if (res.success) {
        setActionNotice(`NapCat 唤醒指令已发送: ${res.data?.detail || '已尝试引导启动'}`);
        setTimeout(() => {
          loadChain();
          onCheckHealth();
        }, 2000);
      } else {
        setActionNotice(`唤醒失败: ${res.error?.message || '未知错误'}`);
      }
    } catch (e: any) {
      setActionNotice(`唤醒异常: ${e?.message || e}`);
    } finally {
      setIsRestartingNapcat(false);
    }
  };

  const handleOpenQqDownload = () => {
    window.open('https://im.qq.com/pcqq/index.shtml', '_blank');
  };

  useEffect(() => {
    loadChain();
  }, []);

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
                  ? '推理服务已就绪'
                  : health.openCode?.path?.includes('未配置')
                  ? '已检测到 CLI (需配置 API 凭证)'
                  : '未运行本地服务'}
              </span>
              {!health.openCode?.ready && (
                <button
                  onClick={onQuickSwitchOpenCode}
                  className="mt-1.5 inline-flex items-center gap-1 text-[11px] text-sky-600 hover:text-sky-700 font-medium hover:underline cursor-pointer"
                >
                  <span>一键切换 Zen 云端免费通道</span>
                  <Zap className="w-3 h-3 text-amber-500" />
                </button>
              )}
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
                {health.isAllReady ? '全部前置条件已满足' : '存在未就绪项，点击各条目一键修复'}
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
              从协议端到界面的 8 个环节逐个体检。若链路断裂，可直接点击一键唤醒或切换，杜绝未知故障。
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleRestartProtocol}
              disabled={isRestartingNapcat}
              className="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 border border-sky-200 text-xs font-medium disabled:opacity-50 transition-colors shrink-0 cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRestartingNapcat ? 'animate-spin' : ''}`} />
              <span>{isRestartingNapcat ? '唤醒中…' : '一键唤醒协议端'}</span>
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
          <div className="p-3 rounded-xl bg-sky-50 border border-sky-200 text-xs text-sky-800 flex items-center justify-between animate-in fade-in">
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
            </div>
            {chainFirstBreak.label.includes('NapCat') || chainFirstBreak.label.includes('OneBot') ? (
              <button
                onClick={handleRestartProtocol}
                disabled={isRestartingNapcat}
                className="px-3 py-1.5 rounded-lg bg-red-600 hover:bg-red-700 text-white font-medium text-xs shrink-0 shadow-xs transition-colors cursor-pointer"
              >
                一键启动/重启
              </button>
            ) : chainFirstBreak.label.includes('大模型') ? (
              chainFirstBreak.detail.includes('@ opencode') ? (
                <button
                  onClick={onCheckHealth}
                  className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-900 text-white font-medium text-xs shrink-0 shadow-xs transition-colors cursor-pointer"
                >
                  重新嗅探连通性
                </button>
              ) : (
                <button
                  onClick={onQuickSwitchOpenCode}
                  className="px-3 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-700 text-white font-medium text-xs shrink-0 shadow-xs transition-colors cursor-pointer"
                >
                  切至官方免费通道
                </button>
              )
            ) : null}
          </div>
        ) : chainLinks.length > 0 ? (
          <div className="p-3 rounded-xl bg-emerald-50 border border-emerald-100 text-[11px] text-emerald-900">
            全链路通畅，未发现断点。
            {chainLinks.some((l) => l.health === 'unknown') &&
              ' 其中部分环节尚未被验证（需要对应组件运行才能确认）。'}
          </div>
        ) : null}

        <div className="space-y-1.5">
          {chainLinks.map((link) => (
            <div
              key={link.link}
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
                </div>
              </div>

              {/* 1-Click Action Buttons for Links */}
              {link.health !== 'ok' && (
                <div className="shrink-0">
                  {(link.link.includes('napcat') || link.link.includes('one_bot')) && (
                    <button
                      onClick={handleRestartProtocol}
                      disabled={isRestartingNapcat}
                      className="px-2.5 py-1 text-[11px] rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 border border-sky-200 font-medium transition-colors cursor-pointer"
                    >
                      一键拉起
                    </button>
                  )}
                  {link.link.includes('ai_provider') && (
                    link.detail.includes('@ opencode') ? (
                      <button
                        onClick={onCheckHealth}
                        className="px-2.5 py-1 text-[11px] rounded-lg bg-slate-100 text-slate-700 hover:bg-slate-200 border border-slate-200 font-medium transition-colors cursor-pointer"
                      >
                        重新检测
                      </button>
                    ) : (
                      <button
                        onClick={onQuickSwitchOpenCode}
                        className="px-2.5 py-1 text-[11px] rounded-lg bg-sky-50 text-sky-700 hover:bg-sky-100 border border-sky-200 font-medium transition-colors cursor-pointer"
                      >
                        切免费通道
                      </button>
                    )
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
