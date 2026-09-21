import React, { useState } from 'react';
import { Search, Filter, Sparkles, Shield, Clock, Check, MessageSquare } from 'lucide-react';
import type { ContactItemDto } from '@/api/contracts';

interface ContactsViewProps {
  contacts: ContactItemDto[];
  onUpdateMode: (targetId: string, mode: 'auto_reply' | 'copilot' | 'summary_only' | 'ignore') => void;
  onToggleSummaryWhitelist?: (targetId: string, isWhitelist: boolean, intervalHours?: number) => void;
  onOpenChat: (contact: ContactItemDto) => void;
}

export const ContactsView: React.FC<ContactsViewProps> = ({
  contacts,
  onUpdateMode,
  onToggleSummaryWhitelist,
  onOpenChat,
}) => {
  const [search, setSearch] = useState('');
  const [filterType, setFilterType] = useState<'all' | 'msg_whitelist' | 'summary_whitelist' | 'friend' | 'group'>('all');

  const filtered = contacts.filter((c) => {
    const matchesSearch =
      c.name.toLowerCase().includes(search.toLowerCase()) ||
      c.targetId.includes(search);

    let matchesType = true;
    if (filterType === 'msg_whitelist') {
      matchesType = c.rule.mode === 'auto_reply' || c.rule.mode === 'copilot';
    } else if (filterType === 'summary_whitelist') {
      matchesType = !!c.rule.isSummaryWhitelist;
    } else if (filterType === 'friend' || filterType === 'group') {
      matchesType = c.targetType === filterType;
    }

    return matchesSearch && matchesType;
  }).sort((a, b) => {
    if (a.targetId === '1739677116') return -1;
    if (b.targetId === '1739677116') return 1;
    return 0;
  });

  const getMessageBadge = (mode: string) => {
    switch (mode) {
      case 'auto_reply':
        return (
          <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-sky-100 text-sky-800 border border-sky-200">
            <Check className="w-3 h-3 text-sky-600" />
            <span>自动秒回 (消息白名单)</span>
          </span>
        );
      case 'copilot':
        return (
          <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-sky-50 text-sky-700 border border-sky-200/80">
            <Shield className="w-3 h-3 text-sky-500" />
            <span>草稿审核 (消息白名单)</span>
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-normal bg-slate-100 text-slate-500 border border-slate-200">
            <span>未接管 (未入白名单)</span>
          </span>
        );
    }
  };

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-hidden bg-slate-50/50">
      {/* Header Toolbar */}
      <div className="flex items-center justify-between pb-4 border-b border-slate-200/80 gap-4">
        {/* Search */}
        <div className="relative flex-1 max-w-md">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            placeholder="搜索好友昵称、群名称或 QQ 号..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-9 pr-3 py-1.5 text-xs rounded-xl border border-slate-200 bg-white focus:outline-none focus:border-sky-500 text-slate-800 placeholder-slate-400 shadow-2xs"
          />
        </div>

        {/* Dual-Whitelist Filter Tabs */}
        <div className="flex items-center gap-1 p-1 rounded-xl bg-slate-100/90 text-xs">
          {([
            { id: 'all', label: '全部' },
            { id: 'msg_whitelist', label: '消息接管白名单' },
            { id: 'summary_whitelist', label: '群总结白名单' },
            { id: 'friend', label: '好友' },
            { id: 'group', label: '群聊' },
          ] as const).map((t) => (
            <button
              key={t.id}
              onClick={() => setFilterType(t.id)}
              className={`px-3 py-1.5 rounded-lg text-xs transition-colors ${
                filterType === t.id
                  ? 'bg-white text-sky-700 shadow-xs font-semibold'
                  : 'text-slate-600 hover:text-slate-900 font-medium'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
      </div>

      {/* Contacts List */}
      <div className="flex-1 overflow-y-auto pt-3">
        {filtered.length === 0 ? (
          <div className="h-64 flex flex-col items-center justify-center text-slate-400 gap-2">
            <Filter className="w-8 h-8 stroke-1 text-slate-300" />
            <span className="text-xs">暂无匹配联系人或群聊</span>
          </div>
        ) : (
          <div className="space-y-2">
            {filtered.map((contact) => {
              const isGroup = contact.targetType === 'group';
              const isSummaryWhitelisted = !!contact.rule.isSummaryWhitelist;
              const summaryInterval = contact.rule.summaryIntervalHours || 6;
              const isTestUser = contact.targetId === '1739677116';

              return (
                <div
                  key={contact.id}
                  className={`flex items-center justify-between p-3.5 rounded-2xl bg-white border transition-all shadow-2xs ${
                    isTestUser
                      ? 'border-sky-300 ring-2 ring-sky-100 dark:ring-sky-900/30 bg-sky-50/20'
                      : 'border-slate-200/80 hover:border-sky-200'
                  }`}
                >
                  {/* Left: Avatar + Details */}
                  <div 
                    className="flex items-center gap-3 cursor-pointer group flex-1 min-w-0 mr-4"
                    onClick={() => onOpenChat(contact)}
                  >
                    <div className="w-10 h-10 rounded-full bg-sky-50 flex items-center justify-center overflow-hidden shrink-0 border border-sky-100 group-hover:scale-105 transition-transform">
                      {contact.avatarUrl ? (
                        <img
                          src={contact.avatarUrl}
                          alt={contact.name}
                          className="w-full h-full object-cover"
                          onError={(e) => {
                            (e.target as HTMLElement).style.display = 'none';
                          }}
                        />
                      ) : (
                        <span className="text-xs font-semibold text-sky-600">
                          {contact.name.slice(0, 1)}
                        </span>
                      )}
                    </div>
                    <div className="flex flex-col min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-sm font-semibold text-slate-900 group-hover:text-sky-600 transition-colors">
                          {contact.name}
                        </span>
                        {isTestUser && (
                          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-emerald-50 text-emerald-700 border border-emerald-200 flex items-center gap-1 shadow-2xs">
                            <Shield className="w-3 h-3 text-emerald-600" />
                            <span>唯一指定测试联系人</span>
                          </span>
                        )}
                        <span className="text-[10px] px-1.5 py-0.2 rounded bg-slate-100 text-slate-500 font-mono">
                          {isGroup ? '群' : '私聊'}
                        </span>
                        <span className="text-[11px] text-slate-400 font-mono">
                          {contact.targetId}
                        </span>
                      </div>
                      <span className="text-xs text-slate-400 truncate max-w-sm mt-0.5">
                        {contact.lastMessageSnippet || '暂无动态消息'}
                      </span>
                    </div>
                  </div>

                  {/* Right: Dual-Whitelist Controls & Chat Action */}
                  <div className="flex items-center gap-2.5 shrink-0">
                    {/* Open Chat Drawer Button */}
                    <button
                      onClick={() => onOpenChat(contact)}
                      className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-medium border transition-colors shadow-2xs ${
                        isTestUser
                          ? 'bg-sky-600 text-white border-sky-600 hover:bg-sky-700 shadow-sky-200/50'
                          : 'bg-sky-50 text-sky-700 border-sky-200 hover:bg-sky-100'
                      }`}
                      title="打开私聊会话与实时 AI 对话窗口"
                    >
                      <MessageSquare className="w-3.5 h-3.5" />
                      <span>私聊会话</span>
                    </button>

                    {/* 1. Group Summary Whitelist Pill (Groups Only) */}
                    {isGroup && (
                      <div className="flex items-center gap-1.5">
                        <button
                          onClick={() => {
                            if (onToggleSummaryWhitelist) {
                              onToggleSummaryWhitelist(
                                contact.targetId,
                                !isSummaryWhitelisted,
                                summaryInterval
                              );
                            }
                          }}
                          className={`inline-flex items-center gap-1 px-2.5 py-1.5 rounded-xl text-xs font-medium border transition-colors ${
                            isSummaryWhitelisted
                              ? 'bg-sky-50 text-sky-700 border-sky-200 hover:bg-sky-100/70 shadow-2xs'
                              : 'bg-white text-slate-400 border-dashed border-slate-300 hover:text-slate-600 hover:border-slate-400'
                          }`}
                          title={isSummaryWhitelisted ? '点击移出总结白名单' : '点击加入总结白名单'}
                        >
                          <Sparkles className={`w-3 h-3 ${isSummaryWhitelisted ? 'text-sky-600' : 'text-slate-400'}`} />
                          <span>{isSummaryWhitelisted ? `简报白名单 (每${summaryInterval}h)` : '+ 简报白名单'}</span>
                        </button>
                      </div>
                    )}

                    {/* 2. Message Receiving Whitelist Badge & Dropdown */}
                    <div className="flex items-center gap-2">
                      {getMessageBadge(contact.rule.mode)}
                      <select
                        value={contact.rule.mode}
                        onChange={(e) => onUpdateMode(contact.targetId, e.target.value as any)}
                        className="text-xs py-1.5 px-3 rounded-xl border border-slate-200 bg-white text-slate-700 focus:outline-none focus:border-sky-500 shadow-2xs"
                      >
                        <option value="ignore">直通忽略 (未入白名单)</option>
                        <option value="copilot">草稿审核 (消息白名单)</option>
                        <option value="auto_reply">自动秒回 (消息白名单)</option>
                      </select>
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
