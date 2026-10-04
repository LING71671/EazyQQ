import React from 'react';
import { Sparkles, ShieldAlert, Clock, Sliders, Send } from 'lucide-react';
import type { SummaryIntervalType } from '@/api/contracts';

interface SummaryConfigCardProps {
  summaryEnabled: boolean;
  onToggleSummaryEnabled: (enabled: boolean) => void;
  intervalType: SummaryIntervalType;
  onChangeIntervalType: (type: SummaryIntervalType) => void;
  customIntervalMinutes: number;
  onChangeCustomIntervalMinutes: (mins: number) => void;
  slidingWindowHours: number;
  onChangeSlidingWindowHours: (hours: number) => void;
  autoForwardToPhone: boolean;
  onToggleAutoForwardToPhone: (forward: boolean) => void;
  customPrompt: string;
  onChangeCustomPrompt: (prompt: string) => void;
}

export const SummaryConfigCard: React.FC<SummaryConfigCardProps> = ({
  summaryEnabled,
  onToggleSummaryEnabled,
  intervalType,
  onChangeIntervalType,
  customIntervalMinutes,
  onChangeCustomIntervalMinutes,
  slidingWindowHours,
  onChangeSlidingWindowHours,
  autoForwardToPhone,
  onToggleAutoForwardToPhone,
  customPrompt,
  onChangeCustomPrompt,
}) => {
  return (
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
            onChange={(e) => onToggleSummaryEnabled(e.target.checked)}
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
                  onClick={() => onChangeIntervalType(t.id as SummaryIntervalType)}
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
                  onChange={(e) => onChangeCustomIntervalMinutes(Number(e.target.value))}
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
                  onClick={() => onChangeSlidingWindowHours(w.hours)}
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
            onChange={(e) => onToggleAutoForwardToPhone(e.target.checked)}
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
            onChange={(e) => onChangeCustomPrompt(e.target.value)}
            placeholder="输入您希望 AI 侧重提取的简报要求..."
            className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-sky-500 leading-relaxed"
          />
        </div>
      </div>
    </div>
  );
};
