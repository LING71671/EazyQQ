import React from 'react';
import { Sparkles, RefreshCw, AlertCircle, CheckCircle2 } from 'lucide-react';
import type { AiProviderId } from '@/api/contracts';

export const AI_PRESETS: Record<
  AiProviderId,
  { baseUrl: string; model: string; label: string; sub: string; local: boolean }
> = {
  opencode: {
    baseUrl: 'https://opencode.ai/zen/v1',
    model: 'qwen3.8-flash',
    label: 'OpenCode 官方免费/Zen',
    sub: '开源官方免费通道 · 免填 Key',
    local: false,
  },
  ollama: {
    baseUrl: 'http://127.0.0.1:11434/v1',
    model: 'qwen2.5:7b',
    label: '本地 Ollama',
    sub: '完全离线、最省 token',
    local: true,
  },
  lmstudio: {
    baseUrl: 'http://127.0.0.1:1234/v1',
    model: 'local-model',
    label: 'LM Studio',
    sub: '本地 GUI 推理',
    local: true,
  },
  llamacpp: {
    baseUrl: 'http://127.0.0.1:8080/v1',
    model: 'local-model',
    label: 'llama.cpp',
    sub: '轻量本地服务',
    local: true,
  },
  vllm: {
    baseUrl: 'http://127.0.0.1:8000/v1',
    model: 'local-model',
    label: 'vLLM',
    sub: '高吞吐本地推理',
    local: true,
  },
  openai: {
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    label: 'OpenAI 兼容端点',
    sub: '通用 API / 任意第三方模型通道',
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
  const u = url.toLowerCase();
  return (
    u.includes('127.0.0.1') ||
    u.includes('localhost') ||
    u.includes('0.0.0.0') ||
    u.includes('[::1]')
  );
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
  fetchedModels: string[];
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
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
            {AI_PROVIDER_ORDER.map((id) => {
              const preset = AI_PRESETS[id];
              const active = provider === id;
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => onSelectProvider(id)}
                  className={`p-2.5 rounded-xl text-left border transition-all ${
                    active
                      ? 'border-sky-500 bg-sky-50/60 text-sky-900 ring-1 ring-sky-500 shadow-2xs'
                      : 'border-slate-200 bg-white hover:border-slate-300 text-slate-700'
                  }`}
                >
                  <span className="text-xs font-semibold flex items-center gap-1.5 flex-wrap">
                    {preset.label}
                    {id === 'opencode' && (
                      <span className="text-[9px] px-1 py-px rounded bg-sky-100 text-sky-700 font-medium">
                        官方推荐
                      </span>
                    )}
                    {preset.local && (
                      <span className="text-[9px] px-1 py-px rounded bg-emerald-100 text-emerald-700 font-medium">
                        本地
                      </span>
                    )}
                  </span>
                  <span className="text-[10px] text-slate-400 block mt-0.5">{preset.sub}</span>
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
                placeholder="可输入或直接点击下方模型..."
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
              <datalist id="dynamic-models-list">
                {fetchedModels.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
            </div>
          </div>

          {/* Dynamic live models chips */}
          {fetchedModels.length > 0 && (
            <div className="p-3 rounded-xl bg-slate-50 border border-slate-200/80 space-y-2">
              <div className="flex items-center justify-between text-[11px]">
                <span className="font-semibold text-slate-700 flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                  <span>接口实时可用模型 (共 {fetchedModels.length} 个 · 点击选用)：</span>
                </span>
                {provider === 'opencode' && (
                  <span className="text-[10px] text-emerald-700 bg-emerald-100/70 font-medium px-1.5 py-0.5 rounded">
                    已高亮 OpenCode 官方免费模型
                  </span>
                )}
              </div>
              <div className="max-h-32 overflow-y-auto pr-1 flex flex-wrap gap-1.5">
                {fetchedModels.map((m, idx) => {
                  const isFree =
                    m.toLowerCase().includes('free') ||
                    m.toLowerCase().includes('flash') ||
                    m.toLowerCase().includes('zen');
                  const isSelected = model === m;
                  return (
                    <button
                      key={m}
                      type="button"
                      onClick={() => onChangeModel(m)}
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
                          免费
                        </span>
                      )}
                      {!isFree && idx === 0 && (
                        <span className="text-[9px] bg-sky-600 text-white px-1 py-px rounded font-semibold">
                          推荐
                        </span>
                      )}
                      <span>{m}</span>
                    </button>
                  );
                })}
              </div>
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
                  OpenCode 官方凭证 (API Key)：
                </label>
                <span className="text-[10px] text-emerald-700 bg-emerald-100/80 px-2 py-0.5 rounded-full font-medium flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3 text-emerald-600" />
                  <span>已自动识别本机 OpenCode Key</span>
                </span>
              </div>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => onChangeApiKey(e.target.value)}
                placeholder="留空自动读取本机 auth.json 密钥 (推荐)"
                className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs font-mono text-slate-800 focus:outline-none focus:border-sky-500"
              />
              <div className="text-[11px] text-slate-500 leading-relaxed">
                应用已自动载入 OpenCode 官方凭证，无需手动填 Key。官方模型随服务端动态更新，点选上方标签即可无缝切换。
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

          <div className="text-[11px] text-slate-400 leading-relaxed">
            模型不硬编码，实时从供应商端点拉取。使用自建网关或官方渠道均可即时更新；保存后立即生效，无需重启。
          </div>
        </div>
      </div>
    </div>
  );
};
