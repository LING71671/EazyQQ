import React, { useState } from 'react';
import { FileText, Clock, Sparkles, Trash2, Users, ShieldAlert } from 'lucide-react';
import type { GroupSummaryDto, ContactItemDto } from '@/api/contracts';

interface SummariesViewProps {
  summaries: GroupSummaryDto[];
  contacts?: ContactItemDto[];
  onGenerateNow: (targetId: string, hours: number) => void;
  onDeleteSummary?: (id: string) => void;
  isGenerating: boolean;
}

export const SummariesView: React.FC<SummariesViewProps> = ({
  summaries,
  contacts = [],
  onGenerateNow,
  onDeleteSummary,
  isGenerating,
}) => {
  const [selectedHours, setSelectedHours] = useState(6);

  // Filter groups from contacts
  const groups = contacts.filter((c) => c.targetType === 'group');
  const whitelistedGroups = groups.filter((g) => g.rule.isSummaryWhitelist);
  
  // Default target ID to first whitelisted group, or first group, or empty
  const defaultTargetId =
    whitelistedGroups.length > 0
      ? whitelistedGroups[0].targetId
      : groups.length > 0
      ? groups[0].targetId
      : '';

  const [targetId, setTargetId] = useState(defaultTargetId);

  const activeTargetId = targetId || defaultTargetId;

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-hidden bg-slate-50/50">
      {/* Header with trigger toolbar */}
      <div className="flex items-center justify-between pb-4 border-b border-slate-200/80 gap-4">
        <div>
          <h2 className="text-base font-semibold text-slate-900">群聊智能简报</h2>
          <p className="text-xs text-slate-500">
            基于白名单与滑动时间窗口长文提炼，输出决议与待办行动项
          </p>
        </div>

        {/* Generate Controls */}
        <div className="flex items-center gap-2">
          {/* Target Group Selector */}
          {groups.length > 0 ? (
            <select
              value={activeTargetId}
              onChange={(e) => setTargetId(e.target.value)}
              className="text-xs py-1.5 px-3 rounded-xl border border-slate-200 bg-white text-slate-700 focus:outline-none focus:border-sky-500 shadow-2xs max-w-[200px]"
            >
              {groups.map((g) => (
                <option key={g.targetId} value={g.targetId}>
                  {g.rule.isSummaryWhitelist ? '⭐ [白名单] ' : ''}
                  {g.name}
                </option>
              ))}
            </select>
          ) : (
            <span className="text-[11px] text-slate-400">未检测到已加入群聊</span>
          )}

          {/* Time Window Selector */}
          <select
            value={selectedHours}
            onChange={(e) => setSelectedHours(Number(e.target.value))}
            className="text-xs py-1.5 px-3 rounded-xl border border-slate-200 bg-white text-slate-700 focus:outline-none focus:border-sky-500 shadow-2xs"
          >
            <option value={2}>过去 2 小时</option>
            <option value={4}>过去 4 小时</option>
            <option value={6}>过去 6 小时 (推荐)</option>
            <option value={12}>过去 12 小时</option>
            <option value={24}>过去 24 小时</option>
          </select>

          <button
            onClick={() => onGenerateNow(activeTargetId, selectedHours)}
            disabled={isGenerating || !activeTargetId}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded-xl text-xs font-semibold bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white shadow-xs transition-colors disabled:opacity-50"
          >
            <Sparkles className={`w-3.5 h-3.5 ${isGenerating ? 'animate-spin' : ''}`} />
            <span>{isGenerating ? '正在梳理脉络...' : '立即提炼简报'}</span>
          </button>
        </div>
      </div>

      {/* Summaries Feed */}
      <div className="flex-1 overflow-y-auto pt-4">
        {summaries.length === 0 ? (
          <div className="h-72 flex flex-col items-center justify-center text-slate-400 gap-3 text-center">
            <div className="w-12 h-12 rounded-2xl bg-sky-50 flex items-center justify-center border border-sky-100 text-sky-600">
              <FileText className="w-6 h-6 stroke-1" />
            </div>
            <div>
              <h3 className="text-xs font-semibold text-slate-700 mb-1">暂无历史简报</h3>
              <p className="text-[11px] text-slate-400 max-w-sm">
                在上方选择目标群聊与滑动时间窗口点击「立即提炼简报」，或在系统设置中启用后台定时自动总结
              </p>
            </div>
          </div>
        ) : (
          <div className="space-y-4 max-w-3xl">
            {summaries.map((s) => (
              <div
                key={s.id}
                className="p-5 rounded-2xl bg-white border border-slate-200/80 hover:border-sky-200 transition-colors shadow-2xs space-y-3"
              >
                <div className="flex items-center justify-between pb-2 border-b border-slate-100">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold text-slate-900">
                      {s.targetName}
                    </span>
                    <span className="text-[11px] px-2 py-0.5 rounded-full bg-sky-50 text-sky-700 border border-sky-100 font-medium">
                      群简报
                    </span>
                  </div>
                  <div className="flex items-center gap-3">
                    <div className="flex items-center gap-1 text-[11px] text-slate-400">
                      <Clock className="w-3.5 h-3.5" />
                      <span>{new Date(s.createdAt).toLocaleString()}</span>
                    </div>
                    {onDeleteSummary && (
                      <button
                        onClick={() => onDeleteSummary(s.id)}
                        className="text-slate-300 hover:text-rose-500 transition-colors"
                        title="删除简报"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    )}
                  </div>
                </div>

                <div className="text-xs text-slate-700 leading-relaxed">
                  <p>{s.summaryText}</p>
                </div>

                {s.keyPoints && s.keyPoints.length > 0 && (
                  <div className="p-3 rounded-xl bg-slate-50 border border-slate-100 text-xs">
                    <span className="font-semibold text-slate-700 block mb-1">关键论点与焦点：</span>
                    <ul className="list-disc list-inside space-y-1 text-slate-600">
                      {s.keyPoints.map((kp, idx) => (
                        <li key={idx}>{kp}</li>
                      ))}
                    </ul>
                  </div>
                )}

                {s.decisions && s.decisions.length > 0 && (
                  <div className="p-3 rounded-xl bg-sky-50/70 border border-sky-100 text-xs">
                    <span className="font-semibold text-sky-800 block mb-1">决议与行动项：</span>
                    <ul className="list-disc list-inside space-y-1 text-sky-700">
                      {s.decisions.map((dec, idx) => (
                        <li key={idx}>{dec}</li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
