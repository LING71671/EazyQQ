import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '@/api/client';
import type { ContactItemDto, PendingDraftDto, MessageItemDto, RuleMode } from '@/api/contracts';
import type { RuleTriggerPatch } from '@/views/ContactsView';

interface UseChatManagerOptions {
  myQqNumber?: string;
}

export function useChatManager(options?: UseChatManagerOptions) {
  const [contacts, setContacts] = useState<ContactItemDto[]>([]);
  const [drafts, setDrafts] = useState<PendingDraftDto[]>([]);
  const [selectedChatContact, setSelectedChatContact] = useState<ContactItemDto | null>(null);
  const [isChatDrawerOpen, setIsChatDrawerOpen] = useState(false);
  const [chatMessages, setChatMessages] = useState<MessageItemDto[]>([]);

  const selectedTargetIdRef = useRef<string | null>(null);
  useEffect(() => {
    selectedTargetIdRef.current = selectedChatContact?.targetId || null;
  }, [selectedChatContact]);

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
    let unlistenDraft: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;

    api.onDraftCreated((draft) => {
      setDrafts((prev) => [draft, ...prev.filter((d) => d.id !== draft.id)]);
    }).then((fn) => {
      unlistenDraft = fn;
    });

    api.onMessageReceived((msg) => {
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
      unlistenMsg = fn;
    });

    return () => {
      unlistenDraft?.();
      unlistenMsg?.();
    };
  }, []);

  const handleOpenChat = useCallback(async (contact: ContactItemDto) => {
    setSelectedChatContact(contact);
    setIsChatDrawerOpen(true);
    try {
      const res = await api.getMessages(contact.targetId, 50, 0, contact.targetType);
      if (res.success && res.data) {
        setChatMessages(res.data);
        const newest = res.data.reduce((max, m) => Math.max(max, m.timestamp), 0);
        await api.markRead(contact.targetId, newest || undefined);
        setContacts((prev) =>
          prev.map((c) => (c.targetId === contact.targetId ? { ...c, unreadCount: 0 } : c))
        );
      }
    } catch (err) {
      console.error('Failed to load chat messages:', err);
    }
  }, []);

  const handleCloseChat = useCallback(() => {
    setIsChatDrawerOpen(false);
  }, []);

  const handleSendMessage = useCallback(
    async (_targetType: string, targetId: string, content: string) => {
      try {
        const res = await api.sendMessage(targetId, content);
        if (res.success) {
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
          setChatMessages((prev) => [...prev, newMsg]);
          setContacts((prev) =>
            prev.map((c) =>
              c.targetId === targetId ? { ...c, lastMessageSnippet: content } : c
            )
          );
        }
      } catch (err) {
        console.error('Failed to send message:', err);
      }
    },
    [options?.myQqNumber]
  );

  const handleTriggerAiReply = useCallback(
    async (targetId: string, contextSnippet: string): Promise<string> => {
      try {
        const res = await api.triggerAiReply(targetId, contextSnippet);
        if (res.success && res.data) {
          return res.data;
        }
      } catch (err) {
        console.error('Failed to trigger AI reply:', err);
      }
      return '';
    },
    []
  );

  const handleSendDraft = useCallback(
    async (draftId: string, finalContent?: string) => {
      try {
        const res = await api.sendDraft(draftId, finalContent);
        if (res.success) {
          setDrafts((prev) => prev.filter((d) => d.id !== draftId));
          if (selectedChatContact) {
            const mRes = await api.getMessages(selectedChatContact.targetId);
            if (mRes.success && mRes.data) setChatMessages(mRes.data);
          }
        }
      } catch (e) {
        console.error('Failed to send draft', e);
      }
    },
    [selectedChatContact]
  );

  const handleDismissDraft = useCallback(async (draftId: string) => {
    try {
      await api.dismissDraft(draftId);
      setDrafts((prev) => prev.filter((d) => d.id !== draftId));
    } catch (e) {
      console.error('Failed to dismiss draft', e);
    }
  }, []);

  const handleRegenerateDraft = useCallback(async (draftId: string, instruction?: string) => {
    try {
      const res = await api.regenerateDraft(draftId, instruction);
      if (res.success && res.data) {
        const updated = res.data;
        setDrafts((prev) => prev.map((d) => (d.id === draftId ? updated : d)));
      }
    } catch (e) {
      console.error('Failed to regenerate draft', e);
    }
  }, []);

  const handleUpdateMode = useCallback(async (targetId: string, mode: RuleMode) => {
    setContacts((prev) =>
      prev.map((c) =>
        c.targetId === targetId ? { ...c, rule: { ...c.rule, mode, enabled: mode !== 'ignore' } } : c
      )
    );
    try {
      await api.updateRule({ targetId, mode, enabled: mode !== 'ignore' });
    } catch (e) {
      console.error('Failed to persist rule to SQLite', e);
    }
  }, []);

  const handleUpdateTrigger = useCallback(async (targetId: string, patch: RuleTriggerPatch) => {
    setContacts((prev) =>
      prev.map((c) => (c.targetId === targetId ? { ...c, rule: { ...c.rule, ...patch } } : c))
    );
    try {
      await api.updateRule({ targetId, ...patch });
    } catch (e) {
      console.error('Failed to persist trigger settings', e);
    }
  }, []);

  const handleToggleSummaryWhitelist = useCallback(
    async (targetId: string, isWhitelist: boolean, intervalHours = 6) => {
      setContacts((prev) =>
        prev.map((c) =>
          c.targetId === targetId
            ? {
                ...c,
                rule: {
                  ...c.rule,
                  isSummaryWhitelist: isWhitelist,
                  summaryIntervalHours: intervalHours,
                },
              }
            : c
        )
      );
      try {
        await api.updateRule({
          targetId,
          isSummaryWhitelist: isWhitelist,
          summaryIntervalHours: intervalHours,
        });
      } catch (e) {
        console.error('Failed to persist summary whitelist to SQLite', e);
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
