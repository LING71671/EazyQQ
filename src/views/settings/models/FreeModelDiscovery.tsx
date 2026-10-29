import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { FreeModelsReport, FreeModelProgress } from '@/api/contracts';
import { FREE_DETECTION_STATES, FREE_MODEL_STATES, nativeModelId } from '@/features/models/freeModels';

interface Props {
  selectedModel: string;
  onChoose: (id: string) => void;
  onVerified: (ids: string[]) => void;
}

interface ActiveRequest { id: string; started: boolean; stopped: boolean; unlisten?: () => void }

export function FreeModelDiscovery({ selectedModel, onChoose, onVerified }: Props) {
  const [report, setReport] = useState<FreeModelsReport | null>(null);
  const [progress, setProgress] = useState<FreeModelProgress | null>(null);
  // The initial catalogue effect must finish before an explicit probe can start.
  const [busy, setBusy] = useState(true);
  const [probing, setProbing] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef<ActiveRequest | null>(null);
  const completion = useRef<Promise<void>>(Promise.resolve());
  const verified = useRef(onVerified);
  verified.current = onVerified;

  const detect = useCallback(async (probe: boolean) => {
    if (request.current) return;
    const requestId = crypto.randomUUID();
    const active: ActiveRequest = { id: requestId, started: false, stopped: false };
    request.current = active;
    const previous = completion.current;
    let finish!: () => void;
    completion.current = new Promise<void>(resolve => { finish = resolve; });
    setBusy(true);
    setProbing(probe);
    setStopping(false);
    setError(null);
    setProgress(null);
    setReport(null);
    verified.current([]);
    try {
      await previous;
      if (request.current !== active) return;
      if (probe) {
        const unlisten = await api.onFreeModelProgress(value => {
          if (request.current === active && value.requestId === active.id) setProgress(value);
        });
        if (request.current !== active) { unlisten(); return; }
        active.unlisten = unlisten;
      }
      if (active.stopped) {
        setReport({ runtimeVersion: '', observedAtMs: Date.now(), catalogueIds: [], models: [], credentialsUsed: false,
          source: 'cancelled_before_start', requestId, status: 'cancelled', completed: 0, total: 0,
          catalogueRefreshed: false, error: null, cleanupError: null, history: [] });
        return;
      }
      active.started = true;
      const result = await api.detectFreeModels(probe, requestId);
      if (request.current !== active) return;
      if (!result.success || !result.data) throw new Error(result.error?.message || '免费模型检测失败');
      if (result.data.credentialsUsed) throw new Error('检测使用了凭据，不能确认为免凭据免费模型');
      setReport(result.data);
      if (result.data.status === 'failed') setError(result.data.error || '免费模型检测失败，请重试');
      verified.current(result.data.models.filter(model => model.state === 'available').map(model => model.id));
    } catch (failure) {
      if (request.current === active) setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      active.unlisten?.();
      active.unlisten = undefined;
      if (request.current === active) { request.current = null; setBusy(false); setProbing(false); setStopping(false); }
      finish();
    }
  }, []);

  const stop = async () => {
    const active = request.current;
    if (!active) return;
    active.stopped = true;
    setStopping(true);
    if (!active.started) return;
    try {
      const result = await api.cancelFreeModelDetection(active.id);
      if (!result.success) throw new Error(result.error?.message || '停止检测失败，请重试');
    } catch (failure) {
      if (request.current === active) { setStopping(false); setError(failure instanceof Error ? failure.message : String(failure)); }
    }
  };

  useEffect(() => {
    void detect(false);
    return () => {
      const active = request.current;
      request.current = null;
      active?.unlisten?.();
      if (active) {
        active.unlisten = undefined;
        if (active.started) void api.cancelFreeModelDetection(active.id).catch(() => {});
      }
    };
  }, [detect]);

  const missing = report?.catalogueRefreshed && selectedModel.trim() && !report.catalogueIds.includes(nativeModelId(selectedModel));
  const retired = report?.models.some(model => model.id === nativeModelId(selectedModel) && model.state === 'retired');
  return <section className="border-t border-slate-200 pt-4 space-y-3" aria-label="OpenCode 免费模型检测" aria-busy={busy}>
    <div className="flex items-center justify-between gap-3 flex-wrap">
      <div>
        <h4 className="text-sm font-semibold text-slate-800">免凭据免费模型</h4>
        <p className="text-xs text-slate-600 mt-1">无需账号或 API Key。本次检测通过后才标为可用，结果随供应方变化。</p>
      </div>
      <div className="flex items-center gap-2">
      {busy && probing && <button type="button" disabled={stopping} onClick={() => void stop()}
        className="px-3 py-2 rounded-lg border border-slate-300 text-slate-700 text-xs font-semibold hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:opacity-50">
        {stopping ? '正在停止…' : '停止检测'}
      </button>}
      <button type="button" disabled={busy} onClick={() => void detect(true)}
        className="px-3 py-2 rounded-lg bg-sky-600 text-white text-xs font-semibold hover:bg-sky-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:opacity-50">
        {busy ? '检测中…' : '自动检测免费模型'}
      </button>
      </div>
    </div>
    {busy && <div role="status" aria-live="polite" className="space-y-1 text-xs text-slate-600">
      <span>{stopping ? '正在停止本次检测，保留已完成结果…' : progress ? `${progress.completed}/${progress.total} · ${progress.model.name}` : '正在刷新原生模型目录…'}</span>
      <progress className="w-full accent-sky-600" aria-label="免费模型检测进度" value={progress?.completed} max={progress?.total || 1} />
    </div>}
    {error && <p role="alert" className="text-xs text-red-700">{error}</p>}
    {report?.cleanupError && <p role="alert" className="text-xs text-amber-800">临时目录清理失败：{report.cleanupError}。下次检测前会重试清理。</p>}
    {missing && <p role="status" className="text-xs text-amber-800">当前模型 {selectedModel} 已不在本次原生目录中；配置未被自动替换。</p>}
    {retired && <p role="status" className="text-xs text-amber-800">当前模型 {selectedModel} 已被明确标记停用；配置未被自动替换。</p>}
    {report && <>
      <p role="status" className="text-xs text-slate-600">{FREE_DETECTION_STATES[report.status]} · 已完成 {report.completed}/{report.total} · {new Date(report.observedAtMs).toLocaleString('zh-CN')}{report.runtimeVersion && ` · OpenCode ${report.runtimeVersion}`}</p>
      <ul className="divide-y divide-slate-200 max-h-72 overflow-y-auto">
        {report.models.map(model => <li key={model.id} className="py-2 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs font-medium text-slate-800 break-words">{model.name} <span className="text-slate-600">· {FREE_MODEL_STATES[model.state]}</span></p>
            <p className="text-xs text-slate-600 mt-1 break-words">{model.detail}</p>
          </div>
          {model.state === 'available' && <button type="button" onClick={() => onChoose(model.id)}
            className="shrink-0 text-xs text-sky-700 font-semibold p-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-600">选用</button>}
        </li>)}
      </ul>
      {report.catalogueRefreshed && report.status === 'completed' && report.models.length === 0 && <p className="text-xs text-slate-600">当前目录没有可检测的零定价候选模型。</p>}
      {report.history.length > 0 && <details className="text-xs text-slate-600">
        <summary className="cursor-pointer py-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-600">历史检测证据（不代表本次可用）</summary>
        <div className="max-h-48 overflow-y-auto divide-y divide-slate-200">
          {report.history.map((entry, index) => <section key={`${entry.requestId}-${entry.observedAtMs}-${index}`} className="py-2 space-y-1">
            <p>{FREE_DETECTION_STATES[entry.status]} · {new Date(entry.observedAtMs).toLocaleString('zh-CN')}</p>
            {entry.models.map(model => <p key={model.id} className="break-words">{model.name} · 历史：{FREE_MODEL_STATES[model.state]}</p>)}
            {entry.models.length === 0 && <p>没有已完成的模型结果。</p>}
          </section>)}
        </div>
      </details>}
    </>}
  </section>;
}
