import { useEffect, useState } from 'react';
import { Send, Trash2, RefreshCw, Plus } from 'lucide-react';
import type { PendingDraftDto, ReplyDeliveryReport } from '@/api/contracts';
import { api } from '@/api/client';
import { NativeMessageText } from '@/components/chat/NativeFace';

interface DraftsViewProps {
  drafts: PendingDraftDto[];
  onSendDraft: (draftId: string, content?: string, bubbles?: string[]) => void | Promise<void>;
  onDismissDraft: (draftId: string) => void | Promise<void>;
  onRegenerateDraft: (draftId: string, instruction?: string) => void | Promise<void>;
}

export const draftBubbles = (draft: PendingDraftDto): string[] => draft.replyPlan?.bubbles?.map(bubble => bubble.segments.map(segment => segment.type === 'text' ? segment.text : `[CQ:face,id=${segment.faceKey}]`).join('')) || [draft.generatedContent];
const receiptState = (receipt: ReplyDeliveryReport['receipts'][number]) => receipt.state || receipt.status || 'pending';
const labels: Record<string, string> = { pending: '待发送', sending: '正在发送', sent: '已发送', failed: '发送失败', unknown: '送达未知', cancelled: '已取消', superseded: '已过期' };
const buttonClass = 'rounded-lg border border-slate-200 dark:border-slate-700 px-3 py-2 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-50';

function DraftCard({ draft, onSendDraft, onDismissDraft, onRegenerateDraft }: Omit<DraftsViewProps, 'drafts'> & { draft: PendingDraftDto }) {
  const [bubbles, setBubbles] = useState(() => draftBubbles(draft));
  const [busy, setBusy] = useState(false);
  const [delivery, setDelivery] = useState<ReplyDeliveryReport | null>(draft.delivery || null);
  const [error, setError] = useState<string | null>(null);
  const [receiptIds, setReceiptIds] = useState<Record<string, string>>({});
  const taskDerived = draft.replyToMsgId?.startsWith('task:') || false;
  useEffect(() => { setBubbles(draftBubbles(draft)); }, [draft.generatedContent, draft.replyPlan]);
  useEffect(() => {
    let disposed = false;
    if (draft.delivery) setDelivery(draft.delivery);
    api.manageReplyBatches({ operation: 'get', batchId: draft.id }).then(response => {
      if (!disposed && response.success && response.data) setDelivery(response.data);
    }).catch(() => {});
    return () => { disposed = true; };
  }, [draft.id, draft.delivery]);
  const committed = !!delivery?.receipts.length;
  const unresolved = delivery?.receipts.some(receipt => ['unknown', 'sending'].includes(receiptState(receipt))) || false;
  const blocked = unresolved || delivery?.receipts.some(receipt => ['superseded', 'cancelled'].includes(receiptState(receipt))) || false;
  const invalid = !bubbles.length || bubbles.some(bubble => !bubble.trim() || bubble.length > 2000);
  const act = async (callback: () => void | Promise<void>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try {
      await callback();
      const response = await api.manageReplyBatches({ operation: 'get', batchId: draft.id });
      if (response.success && response.data) setDelivery(response.data);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(false); }
  };
  const reconcile = async (bubbleId: string, notDelivered: boolean) => {
    await act(async () => {
      const messageId = receiptIds[bubbleId]?.trim();
      const response = await api.manageReplyBatches({ operation: 'reconcile', batchId: draft.id, bubbleId, ...(notDelivered ? { confirmedNotDelivered: true } : { deliveredMessageId: messageId }) });
      if (!response.success || !response.data) throw new Error(response.error?.message || '保存送达核对结果失败');
      setDelivery(response.data);
    });
  };
  return <article aria-label={`草稿：${draft.targetName}`} aria-busy={busy} className="space-y-4 rounded-2xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-5 shadow-xs">
    <div className="flex items-center justify-between gap-3"><h3 className="font-semibold text-slate-900 dark:text-slate-100">{draft.targetName} <span className="text-xs font-normal text-slate-500 dark:text-slate-400">{draft.targetType === 'group' ? '群' : '好友'}</span></h3><span className="text-xs text-slate-400 dark:text-slate-500">{draft.modelUsed}</span></div>
    <div className="rounded-xl bg-slate-50 dark:bg-slate-800/60 p-3 text-xs text-slate-600 dark:text-slate-300"><span className="mb-1 block font-semibold">对方原话</span><p className="whitespace-pre-wrap break-words">{draft.incomingMessageSnippet}</p></div>
    {draft.thinkingContent && <details className="text-xs text-slate-500 dark:text-slate-400"><summary className="cursor-pointer">AI 思考过程</summary><p className="mt-2 whitespace-pre-wrap break-words">{draft.thinkingContent}</p></details>}
    {taskDerived && <p className="text-xs text-slate-500 dark:text-slate-400">任务草稿需要重新生成时，请在任务工作台继续修订。</p>}
    <div className="space-y-3" aria-label="回复气泡">
      {bubbles.map((content, index) => {
        const bubbleId = draft.replyPlan?.bubbles[index]?.id || `b${index + 1}`;
        const receipt = delivery?.receipts.find(item => item.bubbleId === bubbleId) || delivery?.receipts[index];
        const state = receipt && receiptState(receipt);
        return <section key={index} className="space-y-2 rounded-xl border border-slate-200 dark:border-slate-700 p-3">
          <div className="flex items-center justify-between"><label htmlFor={`${draft.id}-bubble-${index}`} className="text-xs font-semibold text-slate-500 dark:text-slate-400">第 {index + 1} 条消息</label><button type="button" aria-label={`删除第${index + 1}条消息`} onClick={() => setBubbles(previous => previous.filter((_, position) => position !== index))} disabled={busy || committed || bubbles.length === 1} className="rounded p-1 text-slate-400 dark:text-slate-500 hover:text-rose-600 dark:hover:text-rose-400 disabled:opacity-40"><Trash2 className="h-3.5 w-3.5" /></button></div>
          <textarea id={`${draft.id}-bubble-${index}`} aria-label={index === 0 ? `回复正文：${draft.targetName}` : `回复正文第${index + 1}条：${draft.targetName}`} value={content} onChange={event => setBubbles(previous => previous.map((value, position) => position === index ? event.target.value : value))} rows={2} maxLength={2000} disabled={busy || committed} className="w-full resize-y rounded-lg bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 p-3 text-sm text-slate-800 dark:text-slate-200 focus:border-sky-500 focus:outline-none disabled:bg-slate-50 dark:disabled:bg-slate-800" />
          {content.includes('[CQ:face,') && <div aria-label="QQ表情发送预览" className="whitespace-pre-wrap break-words text-sm text-slate-800 dark:text-slate-200"><NativeMessageText content={content} /></div>}
          {receipt && <div className="space-y-2 text-xs"><p aria-label={`第${index + 1}条送达状态`} className={state === 'unknown' || state === 'failed' ? 'text-amber-700 dark:text-amber-300' : 'text-slate-500 dark:text-slate-400'}>{labels[state!] || state}{receipt.messageId && ` · ${receipt.messageId}`}{receipt.error && ` · ${receipt.error}`}</p>
            {state === 'unknown' && <div className="flex flex-wrap items-center gap-2"><input aria-label={`第${index + 1}条已送达消息ID`} placeholder="核对到的消息 ID" value={receiptIds[bubbleId] || ''} onChange={event => setReceiptIds(previous => ({ ...previous, [bubbleId]: event.target.value }))} disabled={busy} className="min-w-0 rounded-lg bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 border border-slate-200 dark:border-slate-700 p-2" /><button type="button" onClick={() => void reconcile(bubbleId, false)} disabled={busy || !receiptIds[bubbleId]?.trim()} className={buttonClass}>标记已送达</button><button type="button" onClick={() => void reconcile(bubbleId, true)} disabled={busy} className={buttonClass}>已核对未送达</button></div>}
          </div>}
        </section>;
      })}
      <button type="button" onClick={() => setBubbles(previous => [...previous, ''])} disabled={busy || committed || bubbles.length >= 6} className={`${buttonClass} flex items-center gap-1.5`}><Plus className="h-3.5 w-3.5" />添加一条消息</button>
    </div>
    {error && <p role="alert" className="text-xs text-red-700 dark:text-red-300">{error}</p>}
    <div className="flex flex-wrap items-center justify-between gap-2">
      <button type="button" onClick={() => void act(() => onDismissDraft(draft.id))} disabled={busy || unresolved} className="rounded-lg px-3 py-2 text-xs text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/30 disabled:opacity-50">驳回丢弃</button>
      <div className="flex gap-2"><button type="button" onClick={() => void act(() => onRegenerateDraft(draft.id))} disabled={busy || committed || taskDerived} className={`${buttonClass} flex items-center gap-1.5`}><RefreshCw className="h-3.5 w-3.5" />重新生成</button><button type="button" onClick={() => void act(() => onSendDraft(draft.id, bubbles.join('\n\n'), bubbles))} disabled={busy || blocked || invalid} className="flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-xs font-semibold text-white hover:bg-sky-700 disabled:opacity-50"><Send className="h-3.5 w-3.5" />{busy ? '处理中' : committed ? '继续发送未送达消息' : '确认发送'}</button></div>
    </div>
  </article>;
}

export function DraftsView({ drafts, ...actions }: DraftsViewProps) {
  return <div className="flex h-full flex-col overflow-hidden p-6 dark:bg-slate-950">
    <header className="flex items-center justify-between border-b border-slate-200 dark:border-slate-700 pb-4"><h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">待审批草稿箱</h2><span className="text-xs text-slate-500 dark:text-slate-400">待审：{drafts.length}</span></header>
    <div className="flex-1 overflow-y-auto pt-4"><div className="max-w-3xl space-y-4">{drafts.map(draft => <DraftCard key={draft.id} draft={draft} {...actions} />)}</div></div>
  </div>;
}
