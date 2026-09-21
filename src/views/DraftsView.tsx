import React, { useState } from 'react';
import { Send, Trash2, RefreshCw, ChevronDown, ChevronUp, Check, Bot } from 'lucide-react';
import type { PendingDraftDto } from '@/api/contracts';
import emptyDraftsUrl from '@/assets/empty-drafts.svg';

interface DraftsViewProps {
  drafts: PendingDraftDto[];
  onSendDraft: (draftId: string, content?: string) => void;
  onDismissDraft: (draftId: string) => void;
  onRegenerateDraft: (draftId: string, instruction?: string) => void;
}

export const DraftsView: React.FC<DraftsViewProps> = ({
  drafts,
  onSendDraft,
  onDismissDraft,
  onRegenerateDraft,
}) => {
  const [expandedThinking, setExpandedThinking] = useState<Record<string, boolean>>({});
  const [editingContent, setEditingContent] = useState<Record<string, string>>({});

  const toggleThinking = (id: string) => {
    setExpandedThinking((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-slate-200/80 dark:border-slate-800">
        <div>
          <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">待审批草稿箱</h2>
          <p className="text-xs text-slate-500">AI 针对最新消息生成的拟回复，由您把关放行</p>
        </div>
        <span className="text-xs font-semibold px-2.5 py-1 rounded-full bg-amber-50 text-amber-700 dark:bg-amber-950/40 dark:text-amber-300 border border-amber-200/60 dark:border-amber-800/40">
          待审: {drafts.length}
        </span>
      </div>

      {/* Drafts List */}
      <div className="flex-1 overflow-y-auto pt-4">
        {drafts.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-center p-8">
            <img src={emptyDraftsUrl} alt="Empty Drafts" className="w-36 h-36 mb-4 opacity-70" />
            <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200 mb-1">暂无待审核草稿</h3>
            <p className="text-xs text-slate-400 max-w-sm">
              当开启「草稿审核模式」的联系人或群聊发来消息时，AI 生成的回复将呈现在此处
            </p>
          </div>
        ) : (
          <div className="space-y-4 max-w-3xl">
            {drafts.map((draft) => {
              const isThinkingOpen = !!expandedThinking[draft.id];
              const currentText = editingContent[draft.id] ?? draft.generatedContent;

              return (
                <div
                  key={draft.id}
                  className="rounded-2xl bg-white dark:bg-slate-900 border border-slate-200/80 dark:border-slate-800 shadow-xs p-5 space-y-4"
                >
                  {/* Card Header */}
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                        {draft.targetName}
                      </span>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-100 dark:bg-slate-800 text-slate-500 font-mono">
                        {draft.targetType === 'group' ? '群' : '好友'}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-slate-400">
                      <Bot className="w-3.5 h-3.5" />
                      <span>{draft.modelUsed}</span>
                    </div>
                  </div>

                  {/* Incoming Message Snippet */}
                  <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-800/50 border border-slate-100 dark:border-slate-800 text-xs text-slate-600 dark:text-slate-300">
                    <span className="font-semibold text-slate-500 block mb-1">对方原话：</span>
                    <p className="italic">“{draft.incomingMessageSnippet}”</p>
                  </div>

                  {/* Thinking Section (if available) */}
                  {draft.thinkingContent && (
                    <div className="border border-slate-100 dark:border-slate-800 rounded-xl overflow-hidden">
                      <button
                        onClick={() => toggleThinking(draft.id)}
                        className="w-full flex items-center justify-between px-3 py-2 bg-slate-50/60 dark:bg-slate-800/40 text-[11px] font-medium text-slate-500 hover:text-slate-700 dark:hover:text-slate-300"
                      >
                        <span>AI 思考推演过程</span>
                        {isThinkingOpen ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                      </button>
                      {isThinkingOpen && (
                        <div className="p-3 text-xs text-slate-600 dark:text-slate-400 bg-slate-50/20 dark:bg-slate-900/40 border-t border-slate-100 dark:border-slate-800 leading-relaxed font-mono">
                          {draft.thinkingContent}
                        </div>
                      )}
                    </div>
                  )}

                  {/* Draft Editable Area */}
                  <div className="space-y-1">
                    <span className="text-[11px] font-semibold text-slate-500 block">拟回复正文（可直接编辑）：</span>
                    <textarea
                      value={currentText}
                      onChange={(e) => setEditingContent((prev) => ({ ...prev, [draft.id]: e.target.value }))}
                      rows={3}
                      className="w-full p-3 rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-xs text-slate-800 dark:text-slate-200 focus:outline-none focus:border-sky-500 resize-y"
                    />
                  </div>

                  {/* Actions Toolbar */}
                  <div className="flex items-center justify-between pt-1">
                    <button
                      onClick={() => onDismissDraft(draft.id)}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/30 transition-colors"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      <span>驳回丢弃</span>
                    </button>

                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => onRegenerateDraft(draft.id)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors border border-slate-200 dark:border-slate-700"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                        <span>重新生成</span>
                      </button>
                      <button
                        onClick={() => onSendDraft(draft.id, currentText)}
                        className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-semibold bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white shadow-xs transition-colors"
                      >
                        <Send className="w-3.5 h-3.5" />
                        <span>确认发送</span>
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
