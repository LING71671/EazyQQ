import React, { useEffect, useState } from 'react';
import { CheckCircle2 } from 'lucide-react';
import type { 
  AiProviderId, 
  AppConfig, 
  DependencyHealthReport, 
  SummaryIntervalType,
  ModelInfoDto,
} from '@/api/contracts';
import { api } from '@/api/client';
import { WindowBehaviorCard } from '@/views/settings/WindowBehaviorCard';
import { SummaryConfigCard } from '@/views/settings/SummaryConfigCard';
import { AiProviderCard, AI_PRESETS } from '@/views/settings/AiProviderCard';
import { DiagnosticsCard } from '@/views/settings/DiagnosticsCard';
import { QqPathConfigCard } from '@/views/settings/QqPathConfigCard';
import { AppUpdateCard } from '@/views/settings/AppUpdateCard';

interface SettingsViewProps {
  config: AppConfig;
  health: DependencyHealthReport;
  onUpdateConfig: (cfg: Partial<AppConfig>) => void;
  onCheckHealth: () => void;
  onExportDiagnostics: () => void;
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
  const [provider, setProvider] = useState<AiProviderId>(
    (config.ai?.activeProvider as AiProviderId) || 'opencode'
  );
  const [model, setModel] = useState(config.ai?.model || '');
  const [apiKey, setApiKey] = useState(config.ai?.apiKey || '');
  const [baseUrl, setBaseUrl] = useState(config.ai?.baseUrl || '');
  const [providersMap, setProvidersMap] = useState<
    Record<string, { model: string; baseUrl?: string; apiKey?: string }>
  >(config.ai?.providers || {});

  // Dynamic model fetching
  const [fetchedModels, setFetchedModels] = useState<ModelInfoDto[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [fetchModelError, setFetchModelError] = useState<string | null>(null);

  const fetchModelsForEndpoint = async (
    targetProvider: string,
    targetUrl?: string,
    targetKey?: string,
    autoSelectFirstIfEmpty = false
  ) => {
    setFetchingModels(true);
    setFetchModelError(null);
    try {
      const res = await api.fetchProviderModels(targetProvider, targetUrl, targetKey);
      if (res.success && res.data && res.data.length > 0) {
        const modelList = res.data;
        setFetchedModels(modelList);
        if (autoSelectFirstIfEmpty) {
          setModel((cur) => {
            if (!cur.trim()) {
              const preferred = modelList.find((m) => m.isFree) || modelList[0];
              return preferred?.id || '';
            }
            return cur;
          });
        }
      } else {
        setFetchModelError(res.error?.message || '未获取到模型列表');
      }
    } catch (e: any) {
      setFetchModelError(e?.message || '无法连接该端点获取模型');
    } finally {
      setFetchingModels(false);
    }
  };

  const handleSelectProvider = (id: AiProviderId) => {
    const updatedMap = {
      ...providersMap,
      [provider]: {
        model: model.trim(),
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
      },
    };
    setProvidersMap(updatedMap);

    setProvider(id);
    const existing = updatedMap[id];
    const preset = AI_PRESETS[id] || AI_PRESETS.opencode;
    const nextBaseUrl = existing?.baseUrl || preset.baseUrl;
    const nextModel = existing?.model || '';
    const nextKey =
      existing?.apiKey !== undefined ? existing.apiKey : id === 'opencode' ? apiKey : '';

    setBaseUrl(nextBaseUrl);
    setModel(nextModel);
    setApiKey(nextKey);
    fetchModelsForEndpoint(id, nextBaseUrl, nextKey, !nextModel);
  };

  // Summary Settings
  const [summaryEnabled, setSummaryEnabled] = useState(config.summary?.enabled ?? true);
  const [intervalType, setIntervalType] = useState<SummaryIntervalType>(
    config.summary?.intervalType || '6h'
  );
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

  // Window Behavior
  const [minimizeToTray, setMinimizeToTray] = useState(
    config.window?.minimizeToTray ?? false
  );
  const [closeToTray, setCloseToTray] = useState(config.window?.closeToTray ?? true);

  const applyWindowBehavior = (next: { minimizeToTray?: boolean; closeToTray?: boolean }) => {
    const merged = {
      minimizeToTray: next.minimizeToTray ?? minimizeToTray,
      closeToTray: next.closeToTray ?? closeToTray,
    };
    setMinimizeToTray(merged.minimizeToTray);
    setCloseToTray(merged.closeToTray);
    onUpdateConfig({ window: merged });
  };

  const [isSaved, setIsSaved] = useState(false);

  // Sync state whenever async config from SQLite changes
  useEffect(() => {
    const curProvider = (config.ai?.activeProvider as AiProviderId) || 'opencode';
    const savedMap = config.ai?.providers || {};
    setProvidersMap(savedMap);

    const saved = savedMap[curProvider];
    const preset = AI_PRESETS[curProvider] || AI_PRESETS.opencode;

    const curBaseUrl = saved?.baseUrl || config.ai?.baseUrl || preset.baseUrl;
    const curModel = saved?.model || config.ai?.model || '';
    const curKey = saved?.apiKey !== undefined ? saved.apiKey : config.ai?.apiKey || '';

    setProvider(curProvider);
    setModel(curModel);
    setApiKey(curKey);
    setBaseUrl(curBaseUrl);

    fetchModelsForEndpoint(curProvider, curBaseUrl, curKey, !curModel);

    setSummaryEnabled(config.summary?.enabled ?? true);
    setIntervalType(config.summary?.intervalType || '6h');
    setCustomIntervalMinutes(config.summary?.customIntervalMinutes || 360);
    setSlidingWindowHours(config.summary?.slidingWindowHours || 6);
    setAutoForwardToPhone(config.summary?.autoForwardToPhone ?? false);
    setCustomPrompt(
      config.summary?.customPrompt ||
        '请提取群聊中的核心讨论议题、达成的共识决议、待办行动项及关联责任人，输出清晰简洁的结构化简报。'
    );

    setMinimizeToTray(config.window?.minimizeToTray ?? false);
    setCloseToTray(config.window?.closeToTray ?? true);
  }, [config]);

  const handleSaveAll = () => {
    const updatedMap = {
      ...providersMap,
      [provider]: {
        model: model.trim(),
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
      },
    };
    setProvidersMap(updatedMap);

    onUpdateConfig({
      ai: {
        ...config.ai,
        activeProvider: provider,
        model: model.trim(),
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
        providers: updatedMap,
      },
      summary: {
        enabled: summaryEnabled,
        intervalType,
        customIntervalMinutes,
        slidingWindowHours,
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
            className="px-4 py-2 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-xs transition-all flex items-center gap-1.5 cursor-pointer"
          >
            <CheckCircle2 className="w-4 h-4" />
            <span>{isSaved ? '已保存！' : '保存所有设置'}</span>
          </button>
        </div>

        {/* 1. Window & System Tray Behavior Card */}
        <WindowBehaviorCard
          minimizeToTray={minimizeToTray}
          closeToTray={closeToTray}
          onChangeBehavior={applyWindowBehavior}
        />

        {/* 2. Group Summarization Settings Card */}
        <SummaryConfigCard
          summaryEnabled={summaryEnabled}
          onToggleSummaryEnabled={setSummaryEnabled}
          intervalType={intervalType}
          onChangeIntervalType={setIntervalType}
          customIntervalMinutes={customIntervalMinutes}
          onChangeCustomIntervalMinutes={setCustomIntervalMinutes}
          slidingWindowHours={slidingWindowHours}
          onChangeSlidingWindowHours={setSlidingWindowHours}
          autoForwardToPhone={autoForwardToPhone}
          onToggleAutoForwardToPhone={setAutoForwardToPhone}
          customPrompt={customPrompt}
          onChangeCustomPrompt={setCustomPrompt}
        />

        {/* 3. AI Model Provider Selector Card */}
        <AiProviderCard
          provider={provider}
          onSelectProvider={handleSelectProvider}
          baseUrl={baseUrl}
          onChangeBaseUrl={setBaseUrl}
          model={model}
          onChangeModel={setModel}
          apiKey={apiKey}
          onChangeApiKey={setApiKey}
          fetchedModels={fetchedModels}
          fetchingModels={fetchingModels}
          fetchModelError={fetchModelError}
          onRefreshModels={() => fetchModelsForEndpoint(provider, baseUrl, apiKey, true)}
        />

        {/* 4. Pre-flight Health & End-to-end Chain Diagnostics Card */}
        <DiagnosticsCard
          health={health}
          onCheckHealth={onCheckHealth}
          onQuickSwitchOpenCode={() => handleSelectProvider('opencode')}
          onExportDiagnostics={onExportDiagnostics}
          diagnosticsPath={diagnosticsPath}
        />

        {/* 5. QQNT Executable Path Card */}
        <QqPathConfigCard />

        {/* 6. Version Update Card */}
        <AppUpdateCard />
      </div>
    </div>
  );
};
