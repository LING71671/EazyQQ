import React, { useEffect, useState } from 'react';
import {
  Settings,
  Cpu,
  HardDrive,
  Network,
  FileDown,
  CheckCircle2,
  AlertCircle,
  RefreshCw,
  Clock,
  Sparkles,
  ShieldAlert,
  Send,
  Sliders,
} from 'lucide-react';
import type { AppConfig, DependencyHealthReport } from '@/api/contracts';

interface SettingsViewProps {
  config: AppConfig;
  health: DependencyHealthReport;
  onUpdateConfig: (cfg: Partial<AppConfig>) => void;
  onCheckHealth: () => void;
  onExportDiagnostics: () => void;
  /** Set once a diagnostics bundle has been produced, so the path can be shown. */
  diagnosticsPath?: string;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  config,
  health,
  onUpdateConfig,
  onCheckHealth,
  onExportDiagnostics,
  diagnosticsPath,
}) => {
  // AI Settings
  const [provider, setProvider] = useState(config.ai?.activeProvider || 'opencode');
  const [model, setModel] = useState(config.ai?.model || 'opencode-default');
  const [apiKey, setApiKey] = useState(config.ai?.apiKey || '');

  // Summary Settings (Zero hardcoding, completely dynamic)
  const [summaryEnabled, setSummaryEnabled] = useState(config.summary?.enabled ?? true);
  const [intervalType, setIntervalType] = useState(config.summary?.intervalType || '6h');
  const [customIntervalMinutes, setCustomIntervalMinutes] = useState(
    config.summary?.customIntervalMinutes || 360
  );
  const [slidingWindowHours, setSlidingWindowHours] = useState(
    config.summary?.slidingWindowHours || 6
  );
  const [autoForwardToPhone, setAutoForwardToPhone] = useState(
    config.summary?.autoForwardToPhone ?? false
  );
  const [customPrompt, setCustomPrompt] = useState(
    config.summary?.customPrompt ||
      '请提取群聊中的核心讨论议题、达成的共识决议、待办行动项及关联责任人，输出清晰简洁的结构化简报。'
  );

  const [isSaved, setIsSaved] = useState(false);

  // The config arrives asynchronously from SQLite, so the useState initialisers above
  // only ever see the placeholder defaults. Re-seed whenever the real config lands,
  // otherwise the page silently shows (and would then save) wrong values.
  useEffect(() => {
    setProvider(config.ai?.activeProvider || 'opencode');
    setModel(config.ai?.model || 'opencode-default');
    setApiKey(config.ai?.apiKey || '');

    setSummaryEnabled(config.summary?.enabled ?? true);
    setIntervalType(config.summary?.intervalType || '6h');
    setCustomIntervalMinutes(config.summary?.customIntervalMinutes || 360);
    setSlidingWindowHours(config.summary?.slidingWindowHours || 6);
    setAutoForwardToPhone(config.summary?.autoForwardToPhone ?? false);
    setCustomPrompt(
      config.summary?.customPrompt ||
        '请提取群聊中的核心讨论议题、达成的共识决议、待办行动项及关联责任人，输出清晰简洁的结构化简报。'
    );

    setMinimizeToTray(config.window?.minimizeToTray ?? true);
    setCloseToTray(config.window?.closeToTray ?? true);
  }, [config]);

  // Window & Tray Behavior (default: collapse into tray on minimize / close)
  const [minimizeToTray, setMinimizeToTray] = useState(
    config.window?.minimizeToTray ?? true
  );
  const [closeToTray, setCloseToTray] = useState(config.window?.closeToTray ?? true);

  // Tray behavior must take effect immediately, so persist on every toggle
  // instead of waiting for the global "save all" button.
  const applyWindowBehavior = (next: { minimizeToTray?: boolean; closeToTray?: boolean }) => {
    const merged = {
      minimizeToTray: next.minimizeToTray ?? minimizeToTray,
      closeToTray: next.closeToTray ?? closeToTray,
    };
    setMinimizeToTray(merged.minimizeToTray);
    setCloseToTray(merged.closeToTray);
    onUpdateConfig({ window: merged });
  };

  const handleSaveAll = () => {
    onUpdateConfig({
      ai: {
        ...config.ai,
        activeProvider: provider,
        model,
        apiKey: apiKey.trim(),
      },
      summary: {
        ...config.summary,
        enabled: summaryEnabled,
        intervalType,
        customIntervalMinutes: Number(customIntervalMinutes) || 360,
        slidingWindowHours: Number(slidingWindowHours) || 6,
        autoForwardToPhone,
        customPrompt: customPrompt.trim(),
      },
      window: {
        minimizeToTray,
        closeToTray,
      },
    });
    setIsSaved(true);
    setTimeout(() => setIsSaved(false), 2000);
  };

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-y-auto bg-slate-50/50">
      <div className="max-w-3xl space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-slate-900">系统与自动化配置</h2>
            <p className="text-xs text-slate-500">零命令行图形化配置，所有参数即时写入本地 SQLite</p>
          </div>
          <button
            onClick={handleSaveAll}
            className="px-4 py-2 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-xs transition-all flex items-center gap-1.5"
          >
            <CheckCircle2 className="w-4 h-4" />
            <span>{isSaved ? '已保存！' : '保存所有设置'}</span>
          </button>
        </div>

        {/* 1. Window & System Tray Behavior */}
        <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
          <div className="flex items-center justify-between pb-2 border-b border-slate-100">
            <span className="text-sm font-semibold text-slate-900 flex items-center gap-2">
              <Settings className="w-4 h-4 text-sky-600" />
              <span>窗口与系统托盘行为</span>
            </span>
            <span className="text-[11px] text-slate-400">即改即生效，无需重启</span>
          </div>

          <div className="space-y-3 text-xs">
            <div className="p-3 rounded-xl bg-sky-50/70 border border-sky-100 flex items-start gap-2.5">
              <ShieldAlert className="w-4 h-4 text-sky-600 shrink-0 mt-0.5" />
              <div className="text-sky-900 leading-relaxed text-[11px]">
                <strong className="font-semibold block">托盘常驻保护：</strong>
                窗口缩入托盘后，QQ 协议监听、消息接管与定时群总结仍在后台持续运行，不会被中断。
              </div>
            </div>

            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50 border border-slate-100">
              <div className="pr-4">
                <span className="font-semibold block text-slate-800">最小化时缩至系统托盘</span>
                <span className="text-slate-400 text-[11px]">
                  点击最小化按钮时隐藏窗口至右下角托盘，而非保留在任务栏
                </span>
              </div>
              <input
                type="checkbox"
                checked={minimizeToTray}
                onChange={(e) => applyWindowBehavior({ minimizeToTray: e.target.checked })}
                className="w-4 h-4 shrink-0 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
              />
            </div>

            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50 border border-slate-100">
              <div className="pr-4">
                <span className="font-semibold block text-slate-800">关闭时缩至系统托盘</span>
                <span className="text-slate-400 text-[11px]">
                  点击关闭按钮仅隐藏窗口，不退出进程；彻底退出请右键托盘图标选择「退出 EazyQQ」
                </span>
              </div>
              <input
                type="checkbox"
                checked={closeToTray}
                onChange={(e) => applyWindowBehavior({ closeToTray: e.target.checked })}
                className="w-4 h-4 shrink-0 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
              />
            </div>

            <div className="pt-1 text-[11px] text-slate-400 leading-relaxed">
              标题栏空白处可按住拖动窗口，双击标题栏可最大化 / 还原；单击托盘图标即可重新呼出主窗口。
            </div>
          </div>
        </div>

        {/* 2. Dynamic Group Summarization Settings Card */}
        <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
          <div className="flex items-center justify-between pb-2 border-b border-slate-100">
            <span className="text-sm font-semibold text-slate-900 flex items-center gap-2">
              <Sparkles className="w-4 h-4 text-sky-600" />
              <span>群聊自动定时总结与滑动窗口</span>
            </span>
            <label className="flex items-center gap-2 cursor-pointer">
              <span className="text-xs text-slate-500">
                {summaryEnabled ? '定时总结已启用' : '定时总结已暂停'}
              </span>
              <input
                type="checkbox"
                checked={summaryEnabled}
                onChange={(e) => setSummaryEnabled(e.target.checked)}
                className="w-4 h-4 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
              />
            </label>
          </div>

          <div className="space-y-4 text-xs">
            {/* Whitelist Scope Notice */}
            <div className="p-3 rounded-xl bg-sky-50/70 border border-sky-100 flex items-start gap-2.5">
              <ShieldAlert className="w-4 h-4 text-sky-600 shrink-0 mt-0.5" />
              <div className="text-sky-900 leading-relaxed text-[11px]">
                <strong className="font-semibold block">双白名单保护机制：</strong>
                定时总结与即时提炼严格遵循白名单原则，仅处理在「联系人」中已打上【简报白名单】的群聊。其余任何群聊默认拒绝处理，杜绝数据泄露。
              </div>
            </div>

            {/* Interval Selector */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="font-semibold text-slate-700 block mb-1.5 flex items-center gap-1">
                  <Clock className="w-3.5 h-3.5 text-slate-400" />
                  <span>自动总结执行周期：</span>
                </label>
                <div className="grid grid-cols-4 gap-1.5 mb-2">
                  {[
                    { id: '1h', label: '1 小时' },
                    { id: '2h', label: '2 小时' },
                    { id: '4h', label: '4 小时' },
                    { id: '6h', label: '6 小时' },
                    { id: '12h', label: '12 小时' },
                    { id: '24h', label: '24 小时' },
                    { id: 'custom', label: '自定义' },
                  ].map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      onClick={() => setIntervalType(t.id as any)}
                      className={`py-1.5 px-2 rounded-lg text-[11px] font-medium border transition-colors ${
                        intervalType === t.id
                          ? 'border-sky-500 bg-sky-50 text-sky-700 font-semibold shadow-2xs'
                          : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300'
                      }`}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>

                {intervalType === 'custom' && (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-[11px] text-slate-500">自定义执行间隔:</span>
                    <input
                      type="number"
                      min={10}
                      max={1440}
                      value={customIntervalMinutes}
                      onChange={(e) => setCustomIntervalMinutes(Number(e.target.value))}
                      className="w-24 p-1.5 rounded-lg border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
                    />
                    <span className="text-[11px] text-slate-500">分钟</span>
                  </div>
                )}
              </div>

              {/* Sliding Window */}
              <div>
                <label className="font-semibold text-slate-700 block mb-1.5 flex items-center gap-1">
                  <Sliders className="w-3.5 h-3.5 text-slate-400" />
                  <span>滑动提取时间窗口：</span>
                </label>
                <div className="grid grid-cols-3 gap-1.5 mb-2">
                  {[
                    { hours: 2, label: '过去 2 小时' },
                    { hours: 4, label: '过去 4 小时' },
                    { hours: 6, label: '过去 6 小时' },
                    { hours: 12, label: '过去 12 小时' },
                    { hours: 24, label: '过去 24 小时' },
                    { hours: 48, label: '过去 48 小时' },
                  ].map((w) => (
                    <button
                      key={w.hours}
                      type="button"
                      onClick={() => setSlidingWindowHours(w.hours)}
                      className={`py-1.5 px-2 rounded-lg text-[11px] font-medium border transition-colors ${
                        slidingWindowHours === w.hours
                          ? 'border-sky-500 bg-sky-50 text-sky-700 font-semibold shadow-2xs'
                          : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300'
                      }`}
                    >
                      {w.label}
                    </button>
                  ))}
                </div>
                <span className="text-[11px] text-slate-400 block">
                  每次生成简报时回溯分析的消息范围，不留硬编码死角。
                </span>
              </div>
            </div>

            {/* Auto Push to Mobile QQ */}
            <div className="pt-2 border-t border-slate-100 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Send className="w-4 h-4 text-sky-600" />
                <div>
                  <span className="font-semibold block text-slate-800">自动同步推送到手机</span>
                  <span className="text-slate-400 text-[11px]">
                    生成简报后自动静默发送至自己的「我的电脑」或专属接收群
                  </span>
                </div>
              </div>
              <input
                type="checkbox"
                checked={autoForwardToPhone}
                onChange={(e) => setAutoForwardToPhone(e.target.checked)}
                className="w-4 h-4 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
              />
            </div>

            {/* Custom AI Prompt */}
            <div className="pt-2 border-t border-slate-100">
              <label className="font-semibold text-slate-700 block mb-1">
                AI 简报提取提示词 (Prompt Template)：
              </label>
              <textarea
                rows={2}
                value={customPrompt}
                onChange={(e) => setCustomPrompt(e.target.value)}
                placeholder="输入您希望 AI 侧重提取的简报要求..."
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-sky-500 leading-relaxed"
              />
            </div>
          </div>
        </div>

        {/* 3. AI Model Provider Selector */}
        <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
          <h3 className="text-sm font-semibold text-slate-900 pb-2 border-b border-slate-100">
            大模型推理供应源
          </h3>

          <div className="space-y-3">
            <div>
              <label className="text-xs font-semibold text-slate-700 block mb-1">
                选择大脑类型：
              </label>
              <div className="grid grid-cols-3 gap-3">
                {[
                  { id: 'opencode', label: '本地 OpenCode (推荐)', sub: '零配置、免买 Key' },
                  { id: 'deepseek', label: 'DeepSeek 官方 API', sub: '性价比极高' },
                  { id: 'openai', label: 'OpenAI 兼容接口', sub: '通用 API / OneAPI' },
                ].map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => setProvider(p.id as any)}
                    className={`p-3 rounded-xl text-left border transition-all ${
                      provider === p.id
                        ? 'border-sky-500 bg-sky-50/60 text-sky-900 ring-1 ring-sky-500 shadow-2xs'
                        : 'border-slate-200 bg-white hover:border-slate-300 text-slate-700'
                    }`}
                  >
                    <span className="text-xs font-semibold block">{p.label}</span>
                    <span className="text-[10px] text-slate-400">{p.sub}</span>
                  </button>
                ))}
              </div>
            </div>

            {provider !== 'opencode' && (
              <div className="space-y-3 pt-2">
                <div>
                  <label className="text-xs font-semibold text-slate-700 block mb-1">
                    API Key:
                  </label>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-..."
                    className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
                  />
                </div>
              </div>
            )}
          </div>
        </div>

        {/* 4. Pre-flight Dependency Health Card */}
        <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
          <div className="flex items-center justify-between pb-2 border-b border-slate-100">
            <span className="text-sm font-semibold text-slate-900 flex items-center gap-2">
              <Cpu className="w-4 h-4 text-sky-600" />
              <span>前置运行环境状态</span>
            </span>
            <button
              onClick={onCheckHealth}
              className="flex items-center gap-1 text-xs text-sky-600 hover:text-sky-700 transition-colors"
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
              </div>
              {health.qqNt?.ready ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              ) : (
                <AlertCircle className="w-4 h-4 text-amber-500" />
              )}
            </div>

            {/* OpenCode */}
            <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
              <div>
                <span className="font-semibold block text-slate-800">本地 OpenCode</span>
                <span className="text-slate-400 text-[11px]">
                  {health.openCode?.ready ? '已就绪 (免 API 模式)' : '未检测到，自动使用云端'}
                </span>
              </div>
              {health.openCode?.ready ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              ) : (
                <AlertCircle className="w-4 h-4 text-amber-500" />
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
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              ) : (
                <AlertCircle className="w-4 h-4 text-red-500" />
              )}
            </div>

            {/* Overall verdict */}
            <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 flex items-center justify-between">
              <div>
                <span className="font-semibold block text-slate-800">整体就绪状态</span>
                <span className="text-slate-400 text-[11px]">
                  {health.isAllReady ? '全部前置条件已满足' : '存在未就绪项，详见上方条目'}
                </span>
              </div>
              {health.isAllReady ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              ) : (
                <AlertCircle className="w-4 h-4 text-amber-500" />
              )}
            </div>
          </div>
        </div>

        {/* 5. Diagnostics Export */}
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
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 text-xs text-slate-700 hover:bg-slate-100 transition-colors shrink-0"
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
    </div>
  );
};
