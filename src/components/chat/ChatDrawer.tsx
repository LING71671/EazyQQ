import React, { useState, useEffect, useRef } from 'react';
import { usePresence } from '@/hooks/usePresence';
import {
  X, Send, Bot, Shield, User, Copy, Check,
  ArrowDownCircle, RefreshCw, MessageSquare, FileText
} from 'lucide-react';
import type { ContactItemDto, MessageItemDto, RoutingRuleDto } from '@/api/contracts';

interface ChatDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  contact: ContactItemDto | null;
  messages: MessageItemDto[];
  onSendMessage: (targetType: string, targetId: string, content: string) => Promise<void>;
  onTriggerAiReply?: (targetId: string, contextSnippet: string) => Promise<string>;
  onUpdateMode?: (targetId: string, mode: RoutingRuleDto['mode']) => void;
}

interface MessageSegment {
  type: 'text' | 'image' | 'file' | 'face' | 'at' | 'card';
  text?: string;
  url?: string;
  fileName?: string;
  fileSize?: number;
  summary?: string;
  qq?: string;
  title?: string;
  desc?: string;
  icon?: string;
}

function unescapeCq(str: string): string {
  if (!str) return '';
  return str
    .replace(/&amp;/g, '&')
    .replace(/&#91;/g, '[')
    .replace(/&#93;/g, ']')
    .replace(/&#44;/g, ',')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>');
}

function parseParams(rawParams: string): Record<string, string> {
  const params: Record<string, string> = {};
  const paramPairs = rawParams.split(',');
  for (const pair of paramPairs) {
    const eqIdx = pair.indexOf('=');
    if (eqIdx !== -1) {
      const k = pair.slice(0, eqIdx).trim();
      const v = pair.slice(eqIdx + 1);
      params[k] = unescapeCq(v);
    }
  }
  return params;
}

function parseCqMessage(content: string): MessageSegment[] {
  if (!content) return [];
  const segments: MessageSegment[] = [];
  let i = 0;

  while (i < content.length) {
    const cqStart = content.indexOf('[CQ:', i);
    if (cqStart === -1) {
      const text = content.slice(i);
      if (text) segments.push({ type: 'text', text: unescapeCq(text) });
      break;
    }

    if (cqStart > i) {
      const text = content.slice(i, cqStart);
      if (text) segments.push({ type: 'text', text: unescapeCq(text) });
    }

    const colonIdx = cqStart + 4;
    const commaIdx = content.indexOf(',', colonIdx);
    const closeBracket = content.indexOf(']', colonIdx);

    if (closeBracket === -1) {
      segments.push({ type: 'text', text: content.slice(cqStart) });
      break;
    }

    const typeEnd = (commaIdx !== -1 && commaIdx < closeBracket) ? commaIdx : closeBracket;
    const type = content.slice(colonIdx, typeEnd).trim().toLowerCase();

    let cqEnd = -1;
    let paramsStr = '';

    if (type === 'json') {
      // Find start of JSON object: data={
      const dataBrace = content.indexOf('{', typeEnd);
      let jsonEnd = -1;
      if (dataBrace !== -1) {
        let braceCount = 0;
        let inQuote = false;
        let escaped = false;
        for (let j = dataBrace; j < content.length; j++) {
          const ch = content[j];
          if (escaped) {
            escaped = false;
            continue;
          }
          if (ch === '\\') {
            escaped = true;
            continue;
          }
          if (ch === '"') {
            inQuote = !inQuote;
            continue;
          }
          if (!inQuote) {
            if (ch === '{') braceCount++;
            else if (ch === '}') {
              braceCount--;
              if (braceCount === 0) {
                jsonEnd = j;
                break;
              }
            }
          }
        }
      }

      if (jsonEnd !== -1) {
        const nextBracket = content.indexOf(']', jsonEnd);
        cqEnd = nextBracket !== -1 ? nextBracket + 1 : jsonEnd + 1;
        paramsStr = content.slice(typeEnd + 1, nextBracket !== -1 ? nextBracket : jsonEnd + 1);
      } else {
        const nextBracket = content.indexOf(']', typeEnd);
        cqEnd = nextBracket !== -1 ? nextBracket + 1 : content.length;
        paramsStr = content.slice(typeEnd + 1, cqEnd - 1);
      }
    } else if (type === 'xml') {
      const xmlClose = content.indexOf('>]', typeEnd);
      if (xmlClose !== -1) {
        cqEnd = xmlClose + 2;
        paramsStr = content.slice(typeEnd + 1, xmlClose + 1);
      } else {
        const nextBracket = content.indexOf(']', typeEnd);
        cqEnd = nextBracket !== -1 ? nextBracket + 1 : content.length;
        paramsStr = content.slice(typeEnd + 1, cqEnd - 1);
      }
    } else {
      const nextBracket = content.indexOf(']', typeEnd);
      if (nextBracket === -1) {
        segments.push({ type: 'text', text: content.slice(cqStart) });
        break;
      }
      cqEnd = nextBracket + 1;
      paramsStr = content.slice(typeEnd + 1, nextBracket);
    }

    // Process parsed CQ type
    if (type === 'json') {
      let rawJson = paramsStr.startsWith('data=') ? paramsStr.slice(5) : paramsStr;
      rawJson = unescapeCq(rawJson);
      try {
        const parsed = JSON.parse(rawJson);
        const metaObj = parsed.meta || {};
        const metaKeys = Object.keys(metaObj);
        const detail = metaKeys.length > 0 ? (metaObj[metaKeys[0]] || {}) : {};
        const title = detail.title || parsed.prompt || '小程序卡片';
        const desc = detail.desc || detail.preview || '';
        const icon = detail.icon || detail.preview;
        const prompt = unescapeCq(parsed.prompt || '');

        segments.push({
          type: 'card',
          title: unescapeCq(title),
          desc: unescapeCq(desc),
          icon,
          summary: prompt || '[小程序卡片]',
        });
      } catch {
        const promptMatch = rawJson.match(/"prompt":"([^"]+)"/);
        const titleMatch = rawJson.match(/"title":"([^"]+)"/);
        const descMatch = rawJson.match(/"desc":"([^"]+)"/);
        segments.push({
          type: 'card',
          title: titleMatch ? unescapeCq(titleMatch[1]) : '小程序卡片',
          desc: descMatch ? unescapeCq(descMatch[1]) : (promptMatch ? unescapeCq(promptMatch[1]) : '卡片消息'),
          summary: promptMatch ? unescapeCq(promptMatch[1]) : '[小程序卡片]',
        });
      }
    } else if (type === 'image') {
      const params = parseParams(paramsStr);
      const url = params['url'] || (params['file']?.startsWith('http') ? params['file'] : '');
      segments.push({
        type: 'image',
        url,
        summary: params['summary'] || (url ? '图片' : params['file'] || '图片'),
      });
    } else if (type === 'file') {
      const params = parseParams(paramsStr);
      segments.push({
        type: 'file',
        fileName: params['file'] || '文件',
        fileSize: params['file_size'] ? parseInt(params['file_size'], 10) : undefined,
      });
    } else if (type === 'at') {
      const params = parseParams(paramsStr);
      segments.push({
        type: 'at',
        qq: params['qq'] || '',
        text: `@${params['qq'] || '全体成员'}`,
      });
    } else if (type === 'face') {
      segments.push({
        type: 'face',
        text: '[表情]',
      });
    } else if (type === 'record') {
      segments.push({
        type: 'text',
        text: '[语音消息]',
      });
    } else if (type === 'video') {
      segments.push({
        type: 'text',
        text: '[视频消息]',
      });
    } else if (type === 'reply') {
      segments.push({
        type: 'text',
        text: '[回复]',
      });
    } else {
      segments.push({
        type: 'text',
        text: `[${type}]`,
      });
    }

    i = cqEnd;
  }

  return segments.length > 0 ? segments : [{ type: 'text', text: content }];
}

export const ChatDrawer: React.FC<ChatDrawerProps> = ({
  isOpen,
  onClose,
  contact,
  messages,
  onSendMessage,
  onTriggerAiReply,
  onUpdateMode,
}) => {
  const [inputText, setInputText] = useState('');
  const [isSending, setIsSending] = useState(false);
  const [isAiDrafting, setIsAiDrafting] = useState(false);
  const [copiedUin, setCopiedUin] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Deduplicate messages by content & direction within 60s
  const displayMessages = React.useMemo(() => {
    const deduped: MessageItemDto[] = [];
    for (const msg of messages) {
      if (deduped.length > 0) {
        const prev = deduped[deduped.length - 1];
        const sameContent = prev.content === msg.content;
        const sameDir = prev.isFromMe === msg.isFromMe;
        const closeTime = Math.abs(prev.timestamp - msg.timestamp) < 60000;
        if (sameContent && sameDir && closeTime) {
          if (prev.aiReplyStatus === 'none' && msg.aiReplyStatus !== 'none') {
            prev.aiReplyStatus = msg.aiReplyStatus;
          }
          if ((prev.senderName === '好友' || prev.senderName === '我') && msg.senderName !== '好友' && msg.senderName !== '我') {
            prev.senderName = msg.senderName;
          }
          continue;
        }
      }
      deduped.push({ ...msg });
    }
    return deduped;
  }, [messages]);

  // Auto scroll to bottom
  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    if (isOpen) {
      scrollToBottom();
    }
  }, [isOpen, messages]);

  const mounted = usePresence(isOpen && !!contact);
  if (!mounted || !contact) return null;

  const handleSend = async () => {
    const text = inputText.trim();
    if (!text || isSending) return;

    setIsSending(true);
    try {
      await onSendMessage(contact.targetType, contact.targetId, text);
      setInputText('');
    } catch (err) {
      console.error('Failed to send message:', err);
    } finally {
      setIsSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleAskAiDraft = async () => {
    if (isAiDrafting || !onTriggerAiReply) return;
    setIsAiDrafting(true);
    try {
      const lastIncoming = [...messages].reverse().find(m => !m.isFromMe)?.content || '你好！';
      const aiGenerated = await onTriggerAiReply(contact.targetId, lastIncoming);
      if (aiGenerated) {
        setInputText(aiGenerated);
      }
    } catch (err) {
      console.error('Failed to generate AI draft:', err);
    } finally {
      setIsAiDrafting(false);
    }
  };

  const copyUin = () => {
    navigator.clipboard.writeText(contact.targetId);
    setCopiedUin(true);
    setTimeout(() => setCopiedUin(false), 1500);
  };

  const formatTime = (ts: number) => {
    const d = new Date(ts);
    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
  };

  return (
    <div aria-hidden={!isOpen} inert={!isOpen} className="fixed inset-0 z-50 overflow-hidden flex justify-end">
      {/* Backdrop */}
      <div
        data-open={isOpen}
        className="motion-overlay fixed inset-0 bg-slate-900/20"
        onClick={onClose}
      />

      {/* Slide-over panel */}
      <div data-open={isOpen} className="motion-drawer relative w-full max-w-lg bg-white dark:bg-slate-900 h-full shadow-2xl flex flex-col z-10 border-l border-slate-200 dark:border-slate-800">

        {/* Header */}
        <div className="p-4 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between bg-slate-50/50 dark:bg-slate-900/50">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-10 h-10 rounded-full bg-sky-100 dark:bg-sky-950/50 text-sky-600 flex items-center justify-center font-bold text-sm overflow-hidden shrink-0 border border-sky-200/60 dark:border-sky-800">
              {contact.avatarUrl ? (
                <img src={contact.avatarUrl} alt={contact.name} className="w-full h-full object-cover" />
              ) : (
                <User className="w-5 h-5" />
              )}
            </div>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100 truncate">
                  {contact.name}
                </h3>
              </div>
              <div className="flex items-center gap-2 text-xs text-slate-400 font-mono">
                <span>QQ: {contact.targetId}</span>
                <button onClick={copyUin} className="hover:text-slate-600 dark:hover:text-slate-300 transition-colors" title="复制 QQ 号">
                  {copiedUin ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                </button>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            {/* Mode selector */}
            {onUpdateMode && (
              <select
                value={contact.rule?.mode || 'ignore'}
                onChange={(e) => onUpdateMode(contact.targetId, e.target.value as any)}
                className="text-xs bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-2 py-1 text-slate-700 dark:text-slate-300 font-medium outline-none focus:border-sky-500"
              >
                <option value="ignore">直通忽略 (未入白名单)</option>
                <option value="copilot">草稿审核模式 (人机协同)</option>
                <option value="auto_reply">全自动接管 (自动秒回)</option>
              </select>
            )}

            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Message Stream */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4 bg-slate-50/30 dark:bg-slate-950/20">
          {displayMessages.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-center p-8 text-slate-400">
              <div className="w-12 h-12 rounded-2xl bg-sky-50 dark:bg-sky-950/40 text-sky-500 flex items-center justify-center mb-3">
                <MessageSquare className="w-6 h-6" />
              </div>
              <h4 className="text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                暂无历史对话记录
              </h4>
              <p className="text-[11px] text-slate-400 max-w-xs">
                在此发送一条私聊消息，或等待对方在 QQ 发来信息，即可实时在此查看交互与 AI 构思过程
              </p>
            </div>
          ) : (
            displayMessages.map((msg) => {
              const isMe = msg.isFromMe;
              const isAiAuto = msg.aiReplyStatus === 'auto_replied';
              const isAiDraft = msg.aiReplyStatus === 'draft_pending' || (msg as any).msgType === 'ai_draft_sent';

              return (
                <div
                  key={msg.id}
                  className={`flex flex-col ${isMe ? 'items-end' : 'items-start'} gap-1`}
                >
                  {/* Sender & Badge Info */}
                  <div className="flex items-center gap-1.5 text-[10px] text-slate-400 px-1 font-sans">
                    <span>
                      {isMe
                        ? '我'
                        : contact.targetType === 'group'
                          ? (msg.senderName && msg.senderName !== contact.name && msg.senderName !== '好友'
                              ? msg.senderName
                              : (msg.senderId ? `群友 (${msg.senderId})` : '群成员'))
                          : (msg.senderName || contact.name)}
                    </span>
                    {isAiAuto && (
                      <span className="bg-sky-50 text-sky-600 dark:bg-sky-950/50 dark:text-sky-400 font-semibold px-1 py-0.2 rounded text-[9px] border border-sky-200/60 dark:border-sky-800 flex items-center gap-0.5">
                        AI 秒回
                      </span>
                    )}
                    {isAiDraft && (
                      <span className="bg-emerald-50 text-emerald-600 dark:bg-emerald-950/50 dark:text-emerald-400 font-semibold px-1 py-0.2 rounded text-[9px] border border-emerald-200/60 dark:border-emerald-800 flex items-center gap-0.5">
                        <Check className="w-2.5 h-2.5" /> 草稿审核放行
                      </span>
                    )}
                    <span>{formatTime(msg.timestamp)}</span>
                  </div>

                  {/* Message Bubble */}
                  <div
                    className={`max-w-[85%] rounded-2xl p-3 text-xs leading-relaxed shadow-xs select-text ${
                      isMe
                        ? isAiAuto
                          ? 'bg-sky-600 text-white rounded-tr-xs'
                          : isAiDraft
                            ? 'bg-emerald-600 text-white rounded-tr-xs'
                            : 'bg-sky-600 text-white rounded-tr-xs'
                        : 'bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-100 border border-slate-200/80 dark:border-slate-700/80 rounded-tl-xs'
                    }`}
                  >
                    {/* Rich Segments Rendering (CQ codes -> native elements) */}
                    <div className="space-y-1.5">
                      {parseCqMessage(msg.content).map((seg, idx) => {
                        if (seg.type === 'image') {
                          return seg.url ? (
                            <div key={idx} className="rounded-xl overflow-hidden my-1 max-w-[280px]">
                              <img
                                src={seg.url}
                                alt={seg.summary || '图片'}
                                className="w-full max-h-64 object-contain rounded-xl bg-black/5 hover:opacity-95 transition-opacity cursor-pointer"
                                loading="lazy"
                                onError={(e) => {
                                  const target = e.target as HTMLElement;
                                  target.style.display = 'none';
                                  if (target.parentElement) {
                                    target.parentElement.innerHTML = `<span class="text-xs italic opacity-80">[图片: ${seg.summary || '已过期或无法加载'}]</span>`;
                                  }
                                }}
                              />
                            </div>
                          ) : (
                            <span key={idx} className="italic opacity-80 block">
                              [{seg.summary || '图片'}]
                            </span>
                          );
                        }

                        if (seg.type === 'file') {
                          const sizeStr = seg.fileSize
                            ? seg.fileSize > 1024 * 1024
                              ? `${(seg.fileSize / (1024 * 1024)).toFixed(1)} MB`
                              : `${(seg.fileSize / 1024).toFixed(1)} KB`
                            : '';
                          return (
                            <div
                              key={idx}
                              className={`flex items-center gap-3 p-2.5 rounded-xl my-1 border transition-all ${
                                isMe
                                  ? 'bg-white/15 border-white/20 text-white'
                                  : 'bg-slate-50 dark:bg-slate-800/80 border-slate-200 dark:border-slate-700 text-slate-800 dark:text-slate-100'
                              }`}
                            >
                              <div
                                className={`p-2 rounded-lg ${
                                  isMe
                                    ? 'bg-white/20 text-white'
                                    : 'bg-sky-100 dark:bg-sky-950/60 text-sky-600'
                                }`}
                              >
                                <FileText className="w-5 h-5" />
                              </div>
                              <div className="min-w-0 flex-1 text-left">
                                <span
                                  className="text-xs font-semibold block truncate max-w-[200px]"
                                  title={seg.fileName}
                                >
                                  {seg.fileName}
                                </span>
                                {sizeStr && (
                                  <span
                                    className={`text-[10px] block mt-0.5 ${
                                      isMe ? 'text-white/70' : 'text-slate-400 font-mono'
                                    }`}
                                  >
                                    {sizeStr}
                                  </span>
                                )}
                              </div>
                            </div>
                          );
                        }

                        if (seg.type === 'card') {
                          return (
                            <div
                              key={idx}
                              className={`rounded-xl p-3 my-1 border transition-all max-w-[280px] ${
                                isMe
                                  ? 'bg-white/15 border-white/20 text-white'
                                  : 'bg-slate-50 dark:bg-slate-800/80 border-slate-200 dark:border-slate-700 text-slate-800 dark:text-slate-100'
                              }`}
                            >
                              <div className="flex items-center gap-2 mb-1.5">
                                {seg.icon ? (
                                  <img
                                    src={seg.icon}
                                    alt="icon"
                                    className="w-4 h-4 rounded-full object-cover shrink-0"
                                    onError={(e) => {
                                      (e.target as HTMLElement).style.display = 'none';
                                    }}
                                  />
                                ) : null}
                                <span className={`text-[10px] font-semibold truncate ${isMe ? 'text-white/80' : 'text-slate-500'}`}>
                                  {seg.title || 'QQ小程序'}
                                </span>
                              </div>
                              <div className="text-xs font-semibold leading-snug line-clamp-2">
                                {seg.desc || seg.summary}
                              </div>
                            </div>
                          );
                        }

                        if (seg.type === 'at') {
                          return (
                            <span
                              key={idx}
                              className={`font-semibold mx-0.5 px-1 py-0.2 rounded text-[11px] ${
                                isMe
                                  ? 'bg-white/20 text-white'
                                  : 'bg-sky-100 dark:bg-sky-900/50 text-sky-700 dark:text-sky-300'
                              }`}
                            >
                              {seg.text}
                            </span>
                          );
                        }

                        return (
                          <span key={idx} className="whitespace-pre-wrap break-words block">
                            {seg.text}
                          </span>
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* Input & Quick AI Bar */}
        <div className="p-3 border-t border-slate-100 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-2">
          {/* Quick AI Trigger */}
          <div className="flex items-center justify-between">
            <button
              onClick={handleAskAiDraft}
              disabled={isAiDrafting}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium text-sky-600 dark:text-sky-400 bg-sky-50 dark:bg-sky-950/50 hover:bg-sky-100 dark:hover:bg-sky-900/50 border border-sky-200/60 dark:border-sky-800 transition-colors disabled:opacity-50"
            >

              <span>{isAiDrafting ? 'AI 正在推演拟答...' : '让 AI 替我构思一条回复'}</span>
            </button>

            <span className="text-[10px] text-slate-400 font-mono">
              Enter 发送 / Shift+Enter 换行
            </span>
          </div>

          {/* Textarea and Send button */}
          <div className="flex items-end gap-2 bg-slate-50 dark:bg-slate-800/60 rounded-xl p-2 border border-slate-200 dark:border-slate-700 focus-within:border-sky-500 focus-within:bg-white dark:focus-within:bg-slate-800 transition-all">
            <textarea
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={`给 ${contact.name} 发送私聊消息...`}
              rows={2}
              className="flex-1 bg-transparent text-xs text-slate-800 dark:text-slate-100 placeholder-slate-400 resize-none outline-none leading-relaxed"
            />
            <button
              onClick={handleSend}
              disabled={!inputText.trim() || isSending}
              className="p-2 rounded-lg bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white disabled:opacity-40 transition-colors shrink-0 shadow-xs"
              title="发送"
            >
              <Send className={`w-4 h-4 ${isSending ? 'animate-pulse' : ''}`} />
            </button>
          </div>
        </div>

      </div>
    </div>
  );
};
