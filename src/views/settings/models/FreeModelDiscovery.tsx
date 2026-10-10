import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { FreeModelsReport, FreeModelProgress } from '@/api/contracts';
import { FREE_MODEL_STATES, nativeModelId } from '@/features/models/freeModels';

interface Props {
  selectedModel: string;
  onChoose: (id: string) => void;
  onVerified: (ids: string[]) => void;
}

export function FreeModelDiscovery({ selectedModel, onChoose, onVerified }: Props) {
  const [report, setReport] = useState<FreeModelsReport | null>(null);
  const [progress, setProgress] = useState<FreeModelProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef<string | null>(null);
  const verified = useRef(onVerified);
  verified.current = onVerified;

  const detect = useCallback(async (probe: boolean) => {
    const requestId = crypto.randomUUID();
    request.current = requestId;
    setBusy(true);
    setError(null);
    setProgress(null);
    setReport(null);
    verified.current([]);
    let unlisten: (() => void) | undefined;
    try {
      if (probe) unlisten = await api.onFreeModelProgress(value => {
        if (request.current === value.requestId) setProgress(value);
      });
      const result = await api.detectFreeModels(probe, requestId);
      if (request.current !== requestId) return;
      if (!result.success || !result.data) throw new Error(result.error?.message || '免费模型检测失败');
      if (result.data.credentialsUsed) throw new Error('检测使用了凭据，不能确认为免凭据免费模型');
      setReport(result.data);
      verified.current(result.data.models.filter(model => model.state === 'available').map(model => model.id));
    } catch (failure) {
      if (request.current === requestId) setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      unlisten?.();
      if (request.current === requestId) setBusy(false);
    }
  }, []);

  useEffect(() => {
    void detect(false);
    return () => { request.current = null; };
  }, [detect]);

  const missing = report && selectedModel.trim() && !report.catalogueIds.includes(nativeModelId(selectedModel));
  const retired = report?.models.some(model => model.id === nativeModelId(selectedModel) && model.state === 'retired');
  return <section className="border-t border-slate-200 pt-4 space-y-3" aria-label="OpenCode 免费模型检测" aria-busy={busy}>
    <div className="flex items-center justify-between gap-3 flex-wrap">
      <div>
        <h4 className="text-sm font-semibold text-slate-800">免凭据免费模型</h4>
        <p className="text-xs text-slate-600 mt-1">无需账号或 API Key。本次检测通过后才标为可用，结果随供应方变化。</p>
      </div>
      <button type="button" disabled={busy} onClick={() => void detect(true)}
        className="px-3 py-2 rounded-lg bg-sky-600 text-white text-xs font-semibold hover:bg-sky-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:opacity-50">
        {busy ? '检测中…' : '自动检测免费模型'}
      </button>
    </div>
    {busy && <div role="status" aria-live="polite" className="space-y-1 text-xs text-slate-600">
      <span>{progress ? `${progress.completed}/${progress.total} · ${progress.model.name}` : '正在刷新原生模型目录…'}</span>
      <progress className="w-full accent-sky-600" aria-label="免费模型检测进度" value={progress?.completed} max={progress?.total || 1} />
    </div>}
    {error && <p role="alert" className="text-xs text-red-700">{error}</p>}
    {missing && <p role="status" className="text-xs text-amber-800">当前模型 {selectedModel} 已不在本次原生目录中；配置未被自动替换。</p>}
    {retired && <p role="status" className="text-xs text-amber-800">当前模型 {selectedModel} 已被明确标记停用；配置未被自动替换。</p>}
    {report && <>
      <p className="text-xs text-slate-600">OpenCode {report.runtimeVersion} · {new Date(report.observedAtMs).toLocaleString('zh-CN')}</p>
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
      {report.models.length === 0 && <p className="text-xs text-slate-600">当前目录没有可检测的零定价候选模型。</p>}
    </>}
  </section>;
}
