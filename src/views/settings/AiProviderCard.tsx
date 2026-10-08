import React, { useState } from 'react';
import { Sparkles, RefreshCw, AlertCircle, CheckCircle2, Send } from 'lucide-react';
import type { AiProviderId, ModelInfoDto } from '@/api/contracts';
import { api } from '@/api/client';

export const AI_PRESETS: Record<
  AiProviderId,
  { baseUrl: string; label: string; local: boolean }
> = {
  opencode: {
    baseUrl: 'https://opencode.ai/zen/v1',
    label: '本机 OpenCode',
    local: false,
  },
  ollama: {
    baseUrl: 'http://127.0.0.1:11434/v1',
    label: '本地 Ollama',
    local: true,
  },
  lmstudio: {
    baseUrl: 'http://127.0.0.1:1234/v1',
    label: 'LM Studio',
    local: true,
  },
  llamacpp: {
    baseUrl: 'http://127.0.0.1:8080/v1',
    label: 'llama.cpp',
    local: true,
  },
  vllm: {
    baseUrl: 'http://127.0.0.1:8000/v1',
    label: 'vLLM',
    local: true,
  },
  openai: {
    baseUrl: 'https://api.openai.com/v1',
    label: 'OpenAI 兼容端点',
    local: false,
  },
};

export const AI_PROVIDER_ORDER: AiProviderId[] = [
  'opencode',
  'ollama',
  'lmstudio',
  'llamacpp',
  'vllm',
  'openai',
];

export function isLocalEndpoint(url: string): boolean {
  try { return ['127.0.0.1', 'localhost', '0.0.0.0', '[::1]'].includes(new URL(url).hostname); }
  catch { return false; }
}

interface AiProviderCardProps {
  provider: AiProviderId;
  onSelectProvider: (id: AiProviderId) => void;
  baseUrl: string;
  onChangeBaseUrl: (url: string) => void;
  model: string;
  onChangeModel: (model: string) => void;
  apiKey: string;
  onChangeApiKey: (key: string) => void;
  fetchedModels: ModelInfoDto[];
  fetchingModels: boolean;
  fetchModelError: string | null;
  onRefreshModels: () => void;
}

export const AiProviderCard: React.FC<AiProviderCardProps> = ({
  provider,
  onSelectProvider,
  baseUrl,
  onChangeBaseUrl,
  model,
  onChangeModel,
  apiKey,
  onChangeApiKey,
  fetchedModels,
  fetchingModels,
  fetchModelError,
  onRefreshModels,
}) => {
  const [onlyFreeFilter, setOnlyFreeFilter] = useState(false);
  const [testingModel, setTestingModel] = useState(false);
  const [testResult, setTestResult] = useState<{
    success: boolean;
    latencyMs?: number;
    reply?: string;
    reasoning?: string;
    error?: string;
  } | null>(null);

  const isLocal = isLocalEndpoint(baseUrl);

  const isModelFree = (m: ModelInfoDto) => {
    return isLocal || m.isFree;
  };

  const freeModelsCount = fetchedModels.filter(isModelFree).length;
  const modelsToDisplay = onlyFreeFilter ? fetchedModels.filter(isModelFree) : fetchedModels;

  const handleTestModel = async () => {
    if (!model.trim()) {
      setTestResult({
        success: false,
        error: '请先填写或选择要测试的模型名称',
      });
      return;
    }
    setTestingModel(true);
    setTestResult(null);
    try {
      const res = await api.testAiConnection({
        provider,
        modelId: model.trim(),
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
        prompt: '你好',
      });
      if (res.success && res.data) {
        setTestResult({
          success: true,
          latencyMs: res.data.latencyMs,
          reply: res.data.reply || '(无文字回复)',
          reasoning: res.data.reasoning,
        });
      } else {
        setTestResult({
          success: false,
          error: res.error?.message || '测试失败，请检查端点或密钥',
        });
      }
    } catch (err: any) {
      setTestResult({
        success: false,
        error: String(err?.message || err),
      });
    } finally {
      setTestingModel(false);
    }
  };

  return (
    <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
      <div className="flex items-center justify-between pb-2 border-b border-slate-100">
        <h3 className="text-sm font-semibold text-slate-900">大模型推理供应源</h3>
        <span
          className={`text-[10px] px-2 py-0.5 rounded-full border ${
            isLocalEndpoint(baseUrl)
              ? 'bg-emerald-50 text-emerald-700 border-emerald-200'
              : 'bg-amber-50 text-amber-700 border-amber-200'
          }`}
        >
          {isLocalEndpoint(baseUrl) ? '本地端点 · 不消耗云端 token' : '云端端点 · 消耗 token'}
        </span>
      </div>

      <div className="space-y-4">
        <div>
          <label className="text-xs font-semibold text-slate-700 block mb-2">
            选择大脑类型：
          </label>
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
            {AI_PROVIDER_ORDER.map((id) => {
              const preset = AI_PRESETS[id];
              const active = provider === id;
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => onSelectProvider(id)}
                  className={`p-3 rounded-xl text-left border transition-all flex items-center justify-between cursor-pointer ${
                    active
                      ? 'border-sky-500 bg-sky-50/60 text-sky-900 ring-1 ring-sky-500 shadow-2xs font-semibold'
                      : 'border-slate-200 bg-white hover:border-slate-300 text-slate-700'
                  }`}
                >
                  <span className="text-xs flex items-center gap-1.5 flex-wrap">
                    {preset.label}
                  </span>
                  <div className="flex items-center gap-1 shrink-0">
                    {id === 'opencode' && (
                      <span className="text-[9px] px-1.5 py-0.5 rounded bg-sky-100 text-sky-700 font-medium">
                        官方推荐
                      </span>
                    )}
                    {preset.local && (
                      <span className="text-[9px] px-1.5 py-0.5 rounded bg-emerald-100 text-emerald-700 font-medium">
                        本地
                      </span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        <div className="space-y-3 pt-1">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-semibold text-slate-700 block mb-1">
                接口地址 (Base URL)：
              </label>
              <input
                type="text"
                value={baseUrl}
                onChange={(e) => onChangeBaseUrl(e.target.value)}
                placeholder="http://127.0.0.1:11434/v1"
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
            </div>
            <div>
              <div className="flex items-center justify-between mb-1">
                <label className="text-xs font-semibold text-slate-700">
                  模型名称 (Model)：
                </label>
                <button
                  type="button"
                  onClick={onRefreshModels}
                  disabled={fetchingModels}
                  className="flex items-center gap-1 text-[11px] text-sky-600 hover:text-sky-700 transition-colors cursor-pointer disabled:opacity-50"
                >
                  <RefreshCw className={`w-3 h-3 ${fetchingModels ? 'animate-spin' : ''}`} />
                  <span>{fetchingModels ? '拉取中...' : '从接口动态获取可用模型'}</span>
                </button>
              </div>
              <input
                type="text"
                value={model}
                onChange={(e) => onChangeModel(e.target.value)}
                list="dynamic-models-list"
                placeholder="输入模型名称，或点击下方动态获取到的模型标签..."
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
              <datalist id="dynamic-models-list">
                {fetchedModels.map((m) => (
                  <option key={m.id} value={m.id} label={m.name !== m.id ? m.name : undefined} />
                ))}
              </datalist>
            </div>
          </div>

          {/* Dynamic live models chips */}
          {fetchedModels.length > 0 && (
            <div className="p-3 rounded-xl bg-slate-50 border border-slate-200/80 space-y-2.5">
              <div className="flex items-center justify-between text-[11px] gap-2 flex-wrap">
                <span className="font-semibold text-slate-700 flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                  <span>
                    接口实时可用模型 ({onlyFreeFilter ? `筛选 ${modelsToDisplay.length}/共 ${fetchedModels.length} 个` : `共 ${fetchedModels.length} 个`} · 点击选用)：
                  </span>
                </span>
                <div className="flex items-center gap-1.5">
                  {freeModelsCount > 0 && (
                    <button
                      type="button"
                      onClick={() => setOnlyFreeFilter(!onlyFreeFilter)}
                      className={`text-[10px] font-medium px-2 py-0.5 rounded-full border transition-all cursor-pointer ${
                        onlyFreeFilter
                          ? 'bg-emerald-600 text-white border-emerald-600 shadow-2xs'
                          : 'bg-emerald-50 text-emerald-800 border-emerald-200 hover:bg-emerald-100'
                      }`}
                    >
                      {onlyFreeFilter ? '显示全部模型' : `只看免费通道 (${freeModelsCount})`}
                    </button>
                  )}
                </div>
              </div>

              <div className="max-h-36 overflow-y-auto pr-1 flex flex-wrap gap-1.5">
                {modelsToDisplay.map((m, idx) => {
                  const isFree = isModelFree(m);
                  const isSelected = model === m.id || model === m.name;
                  return (
                    <button
                      key={m.id}
                      type="button"
                      onClick={() => onChangeModel(m.id)}
                      className={`px-2.5 py-1 rounded-lg text-[11px] font-mono transition-all flex items-center gap-1.5 cursor-pointer ${
                        isSelected
                          ? 'bg-sky-600 text-white font-semibold shadow-xs ring-1 ring-sky-600'
                          : isFree
                          ? 'bg-emerald-50 text-emerald-800 border border-emerald-300 hover:bg-emerald-100 font-medium'
                          : 'bg-white text-slate-700 border border-slate-200 hover:border-slate-300'
                      }`}
                    >
                      {isFree && (
                        <span className="text-[9px] bg-emerald-600 text-white px-1 py-px rounded font-semibold">
                          {isLocal ? '本地' : '免费'}
                        </span>
                      )}
                      {!isFree && idx === 0 && (
                        <span className="text-[9px] bg-sky-600 text-white px-1 py-px rounded font-semibold">
                          推荐
                        </span>
                      )}
                      <span>{m.name || m.id}</span>
                    </button>
                  );
                })}
              </div>

              <p className="text-[10px] text-slate-400 leading-tight">
                {isLocal
                  ? '当前为本地推理服务，所有模型均在宿主机离线计算，不产生 token 费用。'
                  : '提示：带「免费」徽章为平台公开免配额测试通道；其余商用模型（如 Claude, GPT, Gemini）需在供应方平台拥有调用额度。'}
              </p>
            </div>
          )}

          {fetchModelError && (
            <div className="p-2.5 rounded-lg bg-amber-50 border border-amber-200 text-amber-800 text-[11px] flex items-center gap-1.5">
              <AlertCircle className="w-3.5 h-3.5 shrink-0" />
              <span>动态拉取模型提示: {fetchModelError} (您仍可手动在输入框填写任意模型)</span>
            </div>
          )}

          {/* API Key */}
          {isLocalEndpoint(baseUrl) ? (
            <div className="p-2.5 rounded-lg bg-emerald-50/70 border border-emerald-100 text-[11px] text-emerald-900 leading-relaxed">
              本地端点无需 API Key，请求不会离开本机，适合用来节省 token 费用。
            </div>
          ) : provider === 'opencode' ? (
            <div className="p-3 rounded-xl bg-sky-50/50 border border-sky-100 space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-xs font-semibold text-slate-800">
                  OpenCode API Key（可选）：
                </label>
                <span className="text-[10px] text-emerald-700 bg-emerald-100/80 px-2 py-0.5 rounded-full font-medium flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3 text-emerald-600" />
                  <span>使用本机 OpenCode 运行时</span>
                </span>
              </div>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => onChangeApiKey(e.target.value)}
                placeholder="免费模型可留空；付费模型使用 OpenCode Key"
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
              <div className="text-[11px] text-slate-500 leading-relaxed">
                通过本机 OpenCode 运行模型，免费通道沿用其登录状态与限制。
              </div>
            </div>
          ) : (
            <div>
              <label className="text-xs font-semibold text-slate-700 block mb-1">
                API Key：
              </label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => onChangeApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
            </div>
          )}

          {/* Test Model Action Bar */}
          <div className="pt-3 border-t border-slate-100 space-y-3">
            <div className="flex items-center justify-between gap-3 flex-wrap">
              <div className="text-[11px] text-slate-500">
                测试连通性：向当前模型发送 <span className="px-1.5 py-0.5 rounded bg-slate-100 font-mono text-[10px] text-slate-700 font-medium">"你好"</span> 验证端点可用性与回复内容
              </div>
              <button
                type="button"
                onClick={handleTestModel}
                disabled={testingModel}
                className="px-3.5 py-1.5 rounded-xl bg-sky-600 hover:bg-sky-500 active:bg-sky-700 text-white text-xs font-medium transition-all shadow-xs flex items-center gap-1.5 cursor-pointer disabled:opacity-50 shrink-0"
              >
                <Send className={`w-3.5 h-3.5 ${testingModel ? 'animate-pulse' : ''}`} />
                <span>{testingModel ? '正在发送“你好”测试中...' : '测试模型 (发送“你好”)'}</span>
              </button>
            </div>

            {/* Test Result Display */}
            {testResult && (
              <div
                className={`p-3 rounded-xl border text-xs transition-all ${
                  testResult.success
                    ? 'bg-emerald-50/70 border-emerald-200 text-emerald-950'
                    : 'bg-rose-50/70 border-rose-200 text-rose-950'
                }`}
              >
                <div className="flex items-center justify-between pb-1.5 mb-1.5 border-b border-black/5">
                  <div className="flex items-center gap-1.5 font-semibold text-[11px]">
                    {testResult.success ? (
                      <>
                        <CheckCircle2 className="w-3.5 h-3.5 text-emerald-600 shrink-0" />
                        <span className="text-emerald-800">模型连通成功 · 响应正常</span>
                      </>
                    ) : (
                      <>
                        <AlertCircle className="w-3.5 h-3.5 text-rose-600 shrink-0" />
                        <span className="text-rose-800">模型连通失败</span>
                      </>
                    )}
                  </div>
                  {testResult.latencyMs !== undefined && (
                    <span className="text-[10px] text-emerald-700/80 font-mono">
                      响应耗时: {testResult.latencyMs}ms
                    </span>
                  )}
                </div>

                {testResult.success ? (
                  <div className="space-y-1.5">
                    <div className="text-[11px] text-slate-600 flex items-start gap-1.5">
                      <span className="text-slate-400 shrink-0 font-medium">发送测试:</span>
                      <span className="font-mono text-slate-700 bg-white/70 px-1.5 py-0.5 rounded border border-emerald-100 text-[10px]">
                        你好
                      </span>
                    </div>
                    <div className="text-[11px] flex items-start gap-1.5">
                      <span className="text-emerald-800 font-medium shrink-0">模型回复:</span>
                      <div className="font-sans text-slate-800 bg-white/80 p-2.5 rounded-lg border border-emerald-100 whitespace-pre-wrap leading-relaxed max-h-40 overflow-y-auto flex-1 text-[11px] shadow-2xs">
                        {testResult.reply}
                      </div>
                    </div>
                    {testResult.reasoning && (
                      <div className="text-[10px] text-slate-500 bg-slate-50/80 p-2 rounded-lg border border-slate-200/60 max-h-24 overflow-y-auto">
                        <span className="font-medium text-slate-600 block mb-0.5">思考过程 (Reasoning):</span>
                        <span className="italic">{testResult.reasoning}</span>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="text-[11px] text-rose-700 whitespace-pre-wrap leading-relaxed font-mono">
                    {testResult.error}
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="text-[11px] text-slate-400 leading-relaxed">
            模型不硬编码，实时从供应商端点拉取。使用自建网关或官方渠道均可即时更新；保存后立即生效，无需重启。
          </div>
        </div>
      </div>
    </div>
  );
};
