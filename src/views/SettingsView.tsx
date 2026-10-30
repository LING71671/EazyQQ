import React, { useEffect, useRef, useState } from 'react';
import { CheckCircle2 } from 'lucide-react';
import type { 
  AiProviderId, 
  AppConfig, 
  DependencyHealthReport, 
  SummaryIntervalType,
  ModelInfoDto,
  ContactItemDto,
} from '@/api/contracts';
import { api } from '@/api/client';
import { WindowBehaviorCard } from '@/views/settings/WindowBehaviorCard';
import { SummaryConfigCard } from '@/views/settings/SummaryConfigCard';
import { AiProviderCard, AI_PRESETS } from '@/views/settings/AiProviderCard';
import { DiagnosticsCard } from '@/views/settings/DiagnosticsCard';
import { QqPathConfigCard } from '@/views/settings/QqPathConfigCard';
import { AppUpdateCard } from '@/views/settings/AppUpdateCard';
import { StylePackCard } from '@/views/settings/StylePackCard';
import { PersonaCard } from '@/views/settings/PersonaCard';

interface SettingsViewProps {
  config: AppConfig;
  health: DependencyHealthReport;
  onUpdateConfig: (cfg: Partial<AppConfig>) => Promise<void>;
  onCheckHealth: () => void;
  onExportDiagnostics: () => void;
  diagnosticsPath?: string;
  accountUin?: string;
  contacts?: ContactItemDto[];
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  config,
  health,
  onUpdateConfig,
  onCheckHealth,
  onExportDiagnostics,
  diagnosticsPath,
  accountUin,
  contacts,
}) => {
  // AI Settings
  const [provider, setProvider] = useState<AiProviderId>(
    (config.ai?.activeProvider as AiProviderId) || 'deepseek'
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
  const modelFetchSequence = useRef(0);
  useEffect(() => () => { modelFetchSequence.current++; }, []);

  const fetchModelsForEndpoint = async (
    targetProvider: string,
    targetUrl?: string,
    targetKey?: string,
    autoSelectFirstIfEmpty = false
  ) => {
    const sequence = ++modelFetchSequence.current;
    setFetchedModels([]);
    setFetchModelError(null);
    const nativeOpenCode = targetProvider === 'opencode' && (() => { try { return new URL(targetUrl || '').hostname === 'opencode.ai'; } catch { return false; } })();
    const local = (() => { try { return ['localhost', '127.0.0.1', '0.0.0.0', '[::1]'].includes(new URL(targetUrl || '').hostname); } catch { return false; } })();
    if (!nativeOpenCode && !local && !targetKey?.trim()) {
      setFetchingModels(false);
      return;
    }
    setFetchingModels(true);
    try {
      const res = await api.fetchProviderModels(targetProvider, targetUrl, targetKey);
      if (sequence !== modelFetchSequence.current) return;
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
      if (sequence !== modelFetchSequence.current) return;
      setFetchModelError(e?.message || '无法连接该端点获取模型');
    } finally {
      if (sequence === modelFetchSequence.current) setFetchingModels(false);
    }
  };

  const handleSelectProvider = (id: AiProviderId) => {
    setIsSaved(false);
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
    const preset = AI_PRESETS[id];
    const nextBaseUrl = existing?.baseUrl || preset.baseUrl;
    const nextModel = existing?.model || preset.model || '';
    const nextKey = existing?.apiKey || '';

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

  const applyWindowBehavior = async (next: { minimizeToTray?: boolean; closeToTray?: boolean }) => {
    if (saving.current) return;
    const merged = {
      minimizeToTray: next.minimizeToTray ?? minimizeToTray,
      closeToTray: next.closeToTray ?? closeToTray,
    };
    setMinimizeToTray(merged.minimizeToTray);
    setCloseToTray(merged.closeToTray);
    saving.current = true;
    setIsSaving(true);
    setIsSaved(false);
    setSaveError(null);
    try {
      await onUpdateConfig({ window: merged });
    } catch (error) {
      setSaveError(`窗口设置未保存：${error instanceof Error ? error.message : String(error)}。请重试保存所有设置。`);
    } finally {
      saving.current = false;
      setIsSaving(false);
    }
  };

  const [isSaved, setIsSaved] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const saving = useRef(false);

  // Sync state whenever async config from SQLite changes
  useEffect(() => {
    const curProvider = (config.ai?.activeProvider as AiProviderId) || 'deepseek';
    const savedMap = config.ai?.providers || {};
    setProvidersMap(savedMap);

    const saved = savedMap[curProvider];
    const preset = AI_PRESETS[curProvider];

    const curBaseUrl = config.ai?.baseUrl || saved?.baseUrl || preset?.baseUrl || '';
    const curModel = config.ai?.model || saved?.model || preset?.model || '';
    const curKey = config.ai?.apiKey !== undefined ? config.ai.apiKey : saved?.apiKey || '';

    setProvider(curProvider);
    setModel(curModel);
    setApiKey(curKey);
    setBaseUrl(curBaseUrl);

    void fetchModelsForEndpoint(curProvider, curBaseUrl, curKey, !curModel);
    return () => { modelFetchSequence.current++; };
  }, [config.ai]);

  useEffect(() => {
    setSummaryEnabled(config.summary?.enabled ?? true);
    setIntervalType(config.summary?.intervalType || '6h');
    setCustomIntervalMinutes(config.summary?.customIntervalMinutes || 360);
    setSlidingWindowHours(config.summary?.slidingWindowHours || 6);
    setAutoForwardToPhone(config.summary?.autoForwardToPhone ?? false);
    setCustomPrompt(
      config.summary?.customPrompt ||
        '请提取群聊中的核心讨论议题、达成的共识决议、待办行动项及关联责任人，输出清晰简洁的结构化简报。'
    );
  }, [config.summary]);

  useEffect(() => {
    setMinimizeToTray(config.window?.minimizeToTray ?? false);
    setCloseToTray(config.window?.closeToTray ?? true);
  }, [config.window]);

  const handleSaveAll = async () => {
    if (saving.current) return;
    saving.current = true;
    setIsSaving(true);
    setIsSaved(false);
    setSaveError(null);
    const updatedMap = {
      ...providersMap,
      [provider]: {
        model: model.trim(),
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
      },
    };
    try {
      await onUpdateConfig({
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
      setProvidersMap(updatedMap);
      setIsSaved(true);
    } catch (error) {
      setSaveError(`设置未保存：${error instanceof Error ? error.message : String(error)}。编辑内容已保留，请重试。`);
    } finally {
      saving.current = false;
      setIsSaving(false);
    }
  };

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-y-auto bg-slate-50/50">
      <div className="max-w-3xl space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-slate-900">系统与自动化配置</h2>
            <p className="text-xs text-slate-500">零命令行图形化配置，编辑后点击保存，测试模型不会自动保存设置</p>
          </div>
          <button
            onClick={handleSaveAll}
            disabled={isSaving}
            className="px-4 py-2 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-xs transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600"
          >
            <CheckCircle2 className="w-4 h-4" />
            <span role="status">{isSaving ? '正在保存…' : isSaved ? '已保存' : '保存所有设置'}</span>
          </button>
        </div>

        {saveError && <p role="alert" className="break-words text-sm text-red-700">{saveError}</p>}
        <fieldset disabled={isSaving} onChange={() => setIsSaved(false)} className="min-w-0 space-y-6">
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
          isDraft={provider !== (config.ai?.activeProvider || 'deepseek') || model.trim() !== (config.ai?.model || AI_PRESETS[provider]?.model || '').trim() || baseUrl.trim() !== (config.ai?.baseUrl || AI_PRESETS[provider]?.baseUrl || '').trim() || apiKey.trim() !== (config.ai?.apiKey || '').trim()}
          onSelectProvider={handleSelectProvider}
          baseUrl={baseUrl}
          onChangeBaseUrl={value => { modelFetchSequence.current++; setFetchingModels(false); setFetchedModels([]); setFetchModelError(null); setBaseUrl(value); }}
          model={model}
          onChangeModel={value => { setIsSaved(false); setModel(value); }}
          apiKey={apiKey}
          onChangeApiKey={value => { modelFetchSequence.current++; setFetchingModels(false); setFetchedModels([]); setFetchModelError(null); setApiKey(value); }}
          fetchedModels={fetchedModels}
          fetchingModels={fetchingModels}
          fetchModelError={fetchModelError}
          onRefreshModels={() => fetchModelsForEndpoint(provider, baseUrl, apiKey, true)}
        />
        </fieldset>

        <StylePackCard />
        <PersonaCard accountUin={accountUin} contacts={contacts} />

        {/* 4. Pre-flight Health & End-to-end Chain Diagnostics Card */}
        <DiagnosticsCard
          health={health}
          onCheckHealth={onCheckHealth}
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
