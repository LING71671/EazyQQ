import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '@/api/client';
import type { ContactItemDto, PendingDraftDto, MessageItemDto, RuleMode } from '@/api/contracts';
import type { RuleTriggerPatch } from '@/views/ContactsView';

interface UseChatManagerOptions {
  myQqNumber?: string;
}

const conversationKey = (type: string, id: string) => `${type === 'group' ? 'group' : 'private'}:${id}`;

export function useChatManager(options?: UseChatManagerOptions) {
  const [contacts, setContacts] = useState<ContactItemDto[]>([]);
  const [drafts, setDrafts] = useState<PendingDraftDto[]>([]);
  const [selectedChatContact, setSelectedChatContact] = useState<ContactItemDto | null>(null);
  const [isChatDrawerOpen, setIsChatDrawerOpen] = useState(false);
  const [chatMessages, setChatMessages] = useState<MessageItemDto[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const pendingDrafts = useRef(new Set<string>());

  const selectedTargetIdRef = useRef<string | null>(null);
  const selectedTargetKeyRef = useRef<string | null>(null);
  const chatEpoch = useRef(0);

  const fetchContactsAndDrafts = useCallback(async () => {
    try {
      const [cRes, dRes] = await Promise.allSettled([
        api.getContacts(),
        api.getPendingDrafts(),
      ]);
      if (cRes.status === 'fulfilled' && cRes.value.success && cRes.value.data) {
        setContacts(cRes.value.data.list);
      }
      if (dRes.status === 'fulfilled' && dRes.value.success && dRes.value.data) {
        setDrafts(dRes.value.data);
      }
    } catch (e) {
      console.error('Failed to fetch contacts and drafts', e);
    }
  }, []);

  // Event listeners for incoming messages and drafts
  useEffect(() => {
    let disposed = false;
    let unlistenDraft: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;

    api.onDraftCreated((draft) => {
      if (disposed) return;
      setDrafts((prev) => [draft, ...prev.filter((d) => d.id !== draft.id)]);
    }).then((fn) => {
      if (disposed) fn();
      else unlistenDraft = fn;
    });

    api.onMessageReceived((msg) => {
      if (disposed) return;
      const isOpenTarget = selectedTargetIdRef.current === msg.targetId;

      setChatMessages((prev) => {
        if (isOpenTarget) {
          if (prev.some((m) => m.id === msg.id)) return prev;
          return [...prev, msg];
        }
        return prev;
      });

      if (isOpenTarget) {
        api.markRead(msg.targetId, msg.timestamp).catch(() => {});
      }

      setContacts((prev) =>
        prev.map((c) =>
          c.targetId === msg.targetId
            ? {
                ...c,
                lastMessageSnippet: msg.content,
                unreadCount: isOpenTarget || msg.isFromMe ? c.unreadCount : c.unreadCount + 1,
              }
            : c
        )
      );
    }).then((fn) => {
      if (disposed) fn();
      else unlistenMsg = fn;
    });

    return () => {
      disposed = true;
      unlistenDraft?.();
      unlistenMsg?.();
    };
  }, []);

  const handleOpenChat = useCallback(async (contact: ContactItemDto) => {
    const epoch = ++chatEpoch.current;
    const key = conversationKey(contact.targetType, contact.targetId);
    selectedTargetIdRef.current = contact.targetId;
    selectedTargetKeyRef.current = key;
    setSelectedChatContact(contact);
    setIsChatDrawerOpen(true);
    setChatMessages([]);
    try {
      const res = await api.getMessages(contact.targetId, 50, 0, contact.targetType);
      if (chatEpoch.current !== epoch || selectedTargetKeyRef.current !== key) return;
      if (!res.success || !res.data) throw new Error(res.error?.message || '读取会话历史失败');
      setChatMessages(res.data);
      const newest = res.data.reduce((max, m) => Math.max(max, m.timestamp), 0);
      const read = await api.markRead(contact.targetId, newest || undefined);
      if (chatEpoch.current !== epoch || selectedTargetKeyRef.current !== key) return;
      if (!read.success) throw new Error(read.error?.message || '保存已读状态失败');
      setContacts((prev) =>
        prev.map((c) => (conversationKey(c.targetType, c.targetId) === key ? { ...c, unreadCount: 0 } : c))
      );
    } catch (err) {
      if (chatEpoch.current === epoch && selectedTargetKeyRef.current === key) setActionError(`读取会话失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, []);

  const handleCloseChat = useCallback(() => {
    chatEpoch.current++;
    selectedTargetIdRef.current = null;
    selectedTargetKeyRef.current = null;
    setIsChatDrawerOpen(false);
  }, []);

  const handleSendMessage = useCallback(
    async (targetType: string, targetId: string, content: string) => {
      const epoch = chatEpoch.current;
      const key = conversationKey(targetType, targetId);
      const res = await api.sendMessage(targetId, content, targetType);
      if (!res.success) throw new Error(res.error?.message || '发送消息失败');
      const newMsg: MessageItemDto = {
        id: res.data?.messageId || Date.now().toString(),
        targetId,
        senderId: options?.myQqNumber || 'me',
        senderName: '我',
        content,
        timestamp: Date.now(),
        isFromMe: true,
        aiReplyStatus: 'none',
      };
      if (chatEpoch.current === epoch && selectedTargetKeyRef.current === key) setChatMessages((prev) => [...prev, newMsg]);
      setContacts((prev) =>
        prev.map((c) =>
          conversationKey(c.targetType, c.targetId) === key ? { ...c, lastMessageSnippet: content } : c
        )
      );
    },
    [options?.myQqNumber]
  );

  const handleTriggerAiReply = useCallback(
    async (targetId: string, contextSnippet: string, sourceMessageId?: string, targetType?: string): Promise<string> => {
      const res = await api.triggerAiReply(targetId, contextSnippet, sourceMessageId, targetType);
      if (!res.success || !res.data) throw new Error(res.error?.message || 'AI 构思回复失败');
      return res.data;
    },
    []
  );

  const handleSendDraft = useCallback(
    async (draftId: string, finalContent?: string, finalBubbles?: string[]) => {
      if (pendingDrafts.current.has(draftId)) return;
      pendingDrafts.current.add(draftId);
      const epoch = chatEpoch.current;
      const key = selectedTargetKeyRef.current;
      setActionError(null);
      try {
        const res = finalBubbles ? await api.sendDraft(draftId, finalContent, undefined, finalBubbles) : await api.sendDraft(draftId, finalContent);
        if (!res.success) throw new Error(res.error?.message || '发送草稿失败');
        const delivery = res.data && 'receipts' in res.data ? res.data : res.data && 'delivery' in res.data ? res.data.delivery : undefined;
        if (!delivery || ['sent', 'completed'].includes(delivery.status)) setDrafts((prev) => prev.filter((d) => d.id !== draftId));
        else setDrafts(prev => prev.map(draft => draft.id === draftId ? { ...draft, delivery } : draft));
        if (selectedChatContact && chatEpoch.current === epoch && selectedTargetKeyRef.current === key) {
          const targetId = selectedChatContact.targetId;
          void api.getMessages(targetId).then(response => {
            if (chatEpoch.current === epoch && selectedTargetKeyRef.current === key && response.success && response.data) setChatMessages(response.data);
          }).catch(() => {});
        }
      } catch (e) {
        setActionError(`发送草稿失败：${e instanceof Error ? e.message : String(e)}`);
      } finally {
        pendingDrafts.current.delete(draftId);
      }
    },
    [selectedChatContact]
  );

  const handleDismissDraft = useCallback(async (draftId: string) => {
    if (pendingDrafts.current.has(draftId)) return;
    pendingDrafts.current.add(draftId);
    setActionError(null);
    try {
      const res = await api.dismissDraft(draftId);
      if (!res.success) throw new Error(res.error?.message || '丢弃草稿失败');
      setDrafts((prev) => prev.filter((d) => d.id !== draftId));
    } catch (e) {
      setActionError(`丢弃草稿失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      pendingDrafts.current.delete(draftId);
    }
  }, []);

  const handleRegenerateDraft = useCallback(async (draftId: string, instruction?: string) => {
    if (pendingDrafts.current.has(draftId)) return;
    pendingDrafts.current.add(draftId);
    setActionError(null);
    try {
      const res = await api.regenerateDraft(draftId, instruction);
      if (!res.success || !res.data) throw new Error(res.error?.message || '重新生成草稿失败');
      const updated = res.data;
      setDrafts((prev) => prev.map((d) => (d.id === draftId ? updated : d)));
    } catch (e) {
      setActionError(`重新生成草稿失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      pendingDrafts.current.delete(draftId);
    }
  }, []);

  const handleUpdateMode = useCallback(async (targetId: string, mode: RuleMode) => {
    setActionError(null);
    try {
      const res = await api.updateRule({ targetId, mode, enabled: mode !== 'ignore' });
      if (!res.success || !res.data) throw new Error(res.error?.message || '保存联系人规则失败');
      const rule = res.data;
      setContacts(prev => prev.map(c => c.targetId === targetId ? { ...c, rule: { ...c.rule, ...rule } } : c));
    } catch (e) {
      setActionError(`保存联系人规则失败：${e instanceof Error ? e.message : String(e)}`);
    }
  }, []);

  const handleUpdateTrigger = useCallback(async (targetId: string, patch: RuleTriggerPatch) => {
    setActionError(null);
    try {
      const res = await api.updateRule({ targetId, ...patch });
      if (!res.success || !res.data) throw new Error(res.error?.message || '保存触发条件失败');
      const rule = res.data;
      setContacts(prev => prev.map(c => c.targetId === targetId ? { ...c, rule: { ...c.rule, ...rule } } : c));
    } catch (e) {
      setActionError(`保存触发条件失败：${e instanceof Error ? e.message : String(e)}`);
    }
  }, []);

  const handleToggleSummaryWhitelist = useCallback(
    async (targetId: string, isWhitelist: boolean, intervalHours = 6) => {
      setActionError(null);
      try {
        const res = await api.updateRule({
          targetId,
          isSummaryWhitelist: isWhitelist,
          summaryIntervalHours: intervalHours,
        });
        if (!res.success || !res.data) throw new Error(res.error?.message || '保存简报白名单失败');
        const rule = res.data;
        setContacts(prev => prev.map(c => c.targetId === targetId ? { ...c, rule: { ...c.rule, ...rule } } : c));
      } catch (e) {
        setActionError(`保存简报白名单失败：${e instanceof Error ? e.message : String(e)}`);
      }
    },
    []
  );

  return {
    contacts,
    setContacts,
    drafts,
    setDrafts,
    selectedChatContact,
    isChatDrawerOpen,
    chatMessages,
    actionError,
    clearActionError: () => setActionError(null),
    fetchContactsAndDrafts,
    handleOpenChat,
    handleCloseChat,
    handleSendMessage,
    handleTriggerAiReply,
    handleSendDraft,
    handleDismissDraft,
    handleRegenerateDraft,
    handleUpdateMode,
    handleUpdateTrigger,
    handleToggleSummaryWhitelist,
  };
}
