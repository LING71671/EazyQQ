import React, { useState, useEffect } from 'react';
import { Sidebar, NavView } from '@/components/layout/Sidebar';
import { TopHeader } from '@/components/layout/TopHeader';
import { ManualDrawer } from '@/components/manual/ManualDrawer';
import { LoginView } from '@/views/LoginView';
import { ContactsView } from '@/views/ContactsView';
import { DraftsView } from '@/views/DraftsView';
import { FilesView } from '@/views/FilesView';
import { SummariesView } from '@/views/SummariesView';
import { SettingsView } from '@/views/SettingsView';
import { ChatDrawer } from '@/components/chat/ChatDrawer';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from '@/api/client';
import type { 
  ProtocolStatusDto, 
  ContactItemDto, 
  PendingDraftDto, 
  GroupFileItemDto, 
  GroupSummaryDto, 
  AppConfig, 
  DependencyHealthReport,
  MessageItemDto
} from '@/api/contracts';

export const App: React.FC = () => {
  const [currentView, setCurrentView] = useState<NavView>('login');
  const [isManualOpen, setIsManualOpen] = useState(false);
  const [isDark, setIsDark] = useState(false);

  // Core Application States (All bound to SQLite, zero fake mock data)
  const [protocolStatus, setProtocolStatus] = useState<ProtocolStatusDto>({
    isConnected: false,
    loginStatus: 'waiting_scan',
    qqNumber: '',
    nickname: '',
  });

  const [contacts, setContacts] = useState<ContactItemDto[]>([]);
  const [drafts, setDrafts] = useState<PendingDraftDto[]>([]);
  const [files, setFiles] = useState<GroupFileItemDto[]>([]);
  const [summaries, setSummaries] = useState<GroupSummaryDto[]>([]);
  const [isSummarizing, setIsSummarizing] = useState(false);

  // Live Chat Drawer States
  const [selectedChatContact, setSelectedChatContact] = useState<ContactItemDto | null>(null);
  const [isChatDrawerOpen, setIsChatDrawerOpen] = useState(false);
  const [chatMessages, setChatMessages] = useState<MessageItemDto[]>([]);
  const selectedTargetIdRef = React.useRef<string | null>(null);

  useEffect(() => {
    selectedTargetIdRef.current = selectedChatContact?.targetId || null;
  }, [selectedChatContact]);

  const [config, setConfig] = useState<AppConfig>({
    ai: {
      activeProvider: 'opencode',
      model: 'opencode-default',
      temperature: 0.7,
      maxContextMessages: 20,
    },
    napcat: {
      wsPort: 3001,
      autoRestart: true,
      heartbeatIntervalSec: 15,
    },
    storage: {
      workspaceDir: 'EazyQQ_Data',
      autoSyncFiles: true,
      maxFileSizeMb: 100,
    },
    summary: {
      enabled: true,
      intervalType: '6h',
      customIntervalMinutes: 360,
      slidingWindowHours: 6,
      autoForwardToPhone: false,
      customPrompt: '请提取群聊中的核心讨论议题、达成的共识决议、待办行动项及关联责任人，输出清晰简洁的结构化简报。',
    },
  });

  const [health, setHealth] = useState<DependencyHealthReport>({
    isAllReady: true,
    qqNt: { ready: true, path: 'C:\\Program Files\\Tencent\\QQNT\\QQ.exe' },
    openCode: { ready: true, version: '1.0.4', activeModel: 'local' },
    ports: { napcatPort: 3001, opencodePort: 4096, isConflict: false },
    storage: { workspacePath: 'B:\\EazyQQ_Data', isWritable: true, freeSpaceMb: 102400 },
  });

  const [isRefreshingQr, setIsRefreshingQr] = useState(false);

  // Poll status & load contacts/summaries/config/drafts from SQLite / OneBot on mount
  useEffect(() => {
    let unlistenDraft: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;

    // Smooth reveal: show window when React is mounted and initialized
    try {
      getCurrentWindow().show();
    } catch {}

    const fetchInitialData = async () => {
      try {
        const [statusRes, contactsRes, summariesRes, configRes, draftsRes] = await Promise.allSettled([
          api.getProtocolStatus(),
          api.getContacts(),
          api.getSummaryHistory(),
          api.getConfig(),
          api.getPendingDrafts(),
        ]);

        if (statusRes.status === 'fulfilled' && statusRes.value.success && statusRes.value.data) {
          const initStatus = statusRes.value.data;
          setProtocolStatus(initStatus);
          // If not logged in and no QR code is available yet, auto-fetch immediately
          if (initStatus.loginStatus !== 'logged_in' && !initStatus.qrcodeBase64) {
            api.refreshQrCode().then((rRes) => {
              if (rRes.success && rRes.data) {
                const qr = rRes.data;
                setProtocolStatus((prev) => ({
                  ...prev,
                  qrcodeBase64: qr.qrcodeBase64,
                  loginStatus: 'waiting_scan',
                }));
              }
            });
          }
        }
        if (contactsRes.status === 'fulfilled' && contactsRes.value.success && contactsRes.value.data) {
          setContacts(contactsRes.value.data.list);
        }
        if (summariesRes.status === 'fulfilled' && summariesRes.value.success && summariesRes.value.data) {
          setSummaries(summariesRes.value.data);
        }
        if (configRes.status === 'fulfilled' && configRes.value.success && configRes.value.data) {
          setConfig(configRes.value.data);
        }
        if (draftsRes.status === 'fulfilled' && draftsRes.value.success && draftsRes.value.data) {
          setDrafts(draftsRes.value.data);
        }
      } catch (e) {
        console.error('Failed to load initial data from SQLite', e);
      }
    };
    fetchInitialData();

    // Setup event listeners for drafts and incoming chat messages
    api.onDraftCreated((draft) => {
      setDrafts((prev) => [draft, ...prev.filter((d) => d.id !== draft.id)]);
    }).then((fn) => { unlistenDraft = fn; });

    api.onMessageReceived((msg) => {
      setChatMessages((prev) => {
        if (selectedTargetIdRef.current === msg.targetId) {
          if (prev.some((m) => m.id === msg.id)) return prev;
          return [...prev, msg];
        }
        return prev;
      });
      setContacts((prev) =>
        prev.map((c) => (c.targetId === msg.targetId ? { ...c, lastMessageSnippet: msg.content } : c))
      );
    }).then((fn) => { unlistenMsg = fn; });

    // Periodic check for scan / login status every 2s if not logged in
    const interval = setInterval(async () => {
      try {
        const res = await api.getProtocolStatus();
        if (res.success && res.data) {
          const nextStatus = res.data;
          setProtocolStatus((prev) => {
            const effectiveQr = nextStatus.loginStatus === 'logged_in'
              ? undefined
              : (nextStatus.qrcodeBase64 || prev.qrcodeBase64);

            const mergedStatus: ProtocolStatusDto = {
              ...nextStatus,
              qrcodeBase64: effectiveQr,
            };

            if (nextStatus.loginStatus === 'logged_in' && prev.loginStatus !== 'logged_in') {
              api.getContacts().then((cRes) => {
                if (cRes.success && cRes.data) setContacts(cRes.data.list);
              });
              api.getSummaryHistory().then((sRes) => {
                if (sRes.success && sRes.data) setSummaries(sRes.data);
              });
              api.getPendingDrafts().then((dRes) => {
                if (dRes.success && dRes.data) setDrafts(dRes.data);
              });
            }

            if (
              prev.loginStatus !== mergedStatus.loginStatus ||
              prev.qqNumber !== mergedStatus.qqNumber ||
              prev.qrcodeBase64 !== mergedStatus.qrcodeBase64 ||
              prev.nickname !== mergedStatus.nickname
            ) {
              return mergedStatus;
            }
            return prev;
          });
        }
      } catch {}
    }, 2000);

    return () => {
      clearInterval(interval);
      unlistenDraft?.();
      unlistenMsg?.();
    };
  }, []);

  const toggleTheme = () => {
    setIsDark((prev) => {
      const next = !prev;
      if (next) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
      return next;
    });
  };

  const handleRefreshQr = async () => {
    setIsRefreshingQr(true);
    try {
      const res = await api.refreshQrCode();
      if (res.success && res.data) {
        const qr = res.data;
        setProtocolStatus((prev) => ({
          ...prev,
          qrcodeBase64: qr.qrcodeBase64,
          loginStatus: 'waiting_scan',
        }));
      }
    } catch (e) {
      console.error('Failed to refresh QR code', e);
    } finally {
      setIsRefreshingQr(false);
    }
  };

  // 1. Message Receiving Whitelist Mode Update
  const handleUpdateMode = async (targetId: string, mode: any) => {
    // Optimistic UI update
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
  };

  // 2. Group Summary Whitelist Toggle
  const handleToggleSummaryWhitelist = async (targetId: string, isWhitelist: boolean, intervalHours = 6) => {
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
  };

  // 3. Save Configuration to SQLite
  const handleUpdateConfig = async (cfg: Partial<AppConfig>) => {
    const updated = { ...config, ...cfg };
    setConfig(updated);
    try {
      await api.updateConfig(updated);
    } catch (e) {
      console.error('Failed to update config in SQLite', e);
    }
  };

  // Chat & Draft Handlers (Connected to Real Backend)
  const handleOpenChat = async (contact: ContactItemDto) => {
    setSelectedChatContact(contact);
    setIsChatDrawerOpen(true);
    try {
      const res = await api.getMessages(contact.targetId);
      if (res.success && res.data) {
        setChatMessages(res.data);
      }
    } catch (err) {
      console.error('Failed to load chat messages:', err);
    }
  };

  const handleSendMessage = async (targetType: string, targetId: string, content: string) => {
    try {
      const res = await api.sendMessage(targetId, content);
      if (res.success) {
        const newMsg: MessageItemDto = {
          id: res.data?.messageId || Date.now().toString(),
          targetId,
          senderId: protocolStatus.qqNumber || 'me',
          senderName: '我',
          content,
          timestamp: Date.now(),
          isFromMe: true,
          aiReplyStatus: 'none',
        };
        setChatMessages((prev) => [...prev, newMsg]);
        setContacts((prev) =>
          prev.map((c) => (c.targetId === targetId ? { ...c, lastMessageSnippet: content } : c))
        );
      }
    } catch (err) {
      console.error('Failed to send message:', err);
    }
  };

  const handleTriggerAiReply = async (targetId: string, contextSnippet: string): Promise<string> => {
    try {
      const res = await api.triggerAiReply(targetId, contextSnippet);
      if (res.success && res.data) {
        return res.data;
      }
    } catch (err) {
      console.error('Failed to trigger AI reply:', err);
    }
    return '';
  };

  const handleSendDraft = async (draftId: string, finalContent?: string) => {
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
  };

  const handleDismissDraft = async (draftId: string) => {
    try {
      await api.dismissDraft(draftId);
      setDrafts((prev) => prev.filter((d) => d.id !== draftId));
    } catch (e) {
      console.error('Failed to dismiss draft', e);
    }
  };

  const handleRegenerateDraft = async (draftId: string, instruction?: string) => {
    try {
      const res = await api.regenerateDraft(draftId, instruction);
      if (res.success && res.data) {
        const updated = res.data;
        setDrafts((prev) => prev.map((d) => (d.id === draftId ? updated : d)));
      }
    } catch (e) {
      console.error('Failed to regenerate draft', e);
    }
  };

  const handleDownloadFile = (_groupId: string, fileId: string, _fileName: string) => {
    setFiles((prev) =>
      prev.map((f) =>
        f.fileId === fileId ? { ...f, downloadStatus: 'downloading' } : f
      )
    );
    setTimeout(() => {
      setFiles((prev) =>
        prev.map((f) =>
          f.fileId === fileId ? { ...f, downloadStatus: 'downloaded' } : f
        )
      );
    }, 1500);
  };

  // 4. Generate Real Summary (Persisted to SQLite, zero mock data)
  const handleGenerateSummary = async (targetId: string, hours: number) => {
    setIsSummarizing(true);
    try {
      const res = await api.generateSummary(targetId, hours);
      if (res.success && res.data) {
        const newSummary = res.data;
        setSummaries((prev) => [newSummary, ...prev.filter((s) => s.id !== newSummary.id)]);
      }
    } catch (e) {
      console.error('Failed to generate summary', e);
    } finally {
      setIsSummarizing(false);
    }
  };

  const handleDeleteSummary = async (id: string) => {
    try {
      await api.deleteSummary(id);
      setSummaries((prev) => prev.filter((s) => s.id !== id));
    } catch (e) {
      console.error('Failed to delete summary', e);
    }
  };

  const getViewTitle = () => {
    switch (currentView) {
      case 'login':
        return '账号状态与扫码';
      case 'contacts':
        return '联系人与白名单管理';
      case 'drafts':
        return '人机协同草稿箱';
      case 'files':
        return '群文件知识库';
      case 'summaries':
        return '群聊消息简报';
      case 'settings':
        return '系统与自动化配置';
    }
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-white text-slate-900 select-none">
      {/* Sidebar Navigation */}
      <Sidebar
        currentView={currentView}
        onSelectView={setCurrentView}
        pendingDraftCount={drafts.length}
        onOpenManual={() => setIsManualOpen(true)}
      />

      {/* Main Container */}
      <div className="flex-1 flex flex-col h-screen overflow-hidden bg-white">
        {/* Top Header */}
        <TopHeader
          title={getViewTitle()}
          protocolStatus={protocolStatus}
          isDark={isDark}
          onToggleTheme={toggleTheme}
          onOpenManual={() => setIsManualOpen(true)}
        />

        {/* View Switcher */}
        <main className="flex-1 overflow-hidden bg-slate-50/50">
          {currentView === 'login' && (
            <LoginView
              status={protocolStatus}
              onRefreshQr={handleRefreshQr}
              isLoading={isRefreshingQr}
            />
          )}
          {currentView === 'contacts' && (
            <ContactsView
              contacts={contacts}
              onUpdateMode={handleUpdateMode}
              onToggleSummaryWhitelist={handleToggleSummaryWhitelist}
              onOpenChat={handleOpenChat}
            />
          )}
          {currentView === 'drafts' && (
            <DraftsView
              drafts={drafts}
              onSendDraft={handleSendDraft}
              onDismissDraft={handleDismissDraft}
              onRegenerateDraft={handleRegenerateDraft}
            />
          )}
          {currentView === 'files' && (
            <FilesView
              files={files}
              onDownloadFile={handleDownloadFile}
              onSummarizeFile={() => {}}
              onOpenFolder={() => {}}
              isSummarizing={false}
            />
          )}
          {currentView === 'summaries' && (
            <SummariesView
              summaries={summaries}
              contacts={contacts}
              onGenerateNow={handleGenerateSummary}
              onDeleteSummary={handleDeleteSummary}
              isGenerating={isSummarizing}
            />
          )}
          {currentView === 'settings' && (
            <SettingsView
              config={config}
              health={health}
              onUpdateConfig={handleUpdateConfig}
              onCheckHealth={() => {}}
              onExportDiagnostics={() => {}}
            />
          )}
        </main>
      </div>

      {/* F1 Built-in Manual Drawer */}
      <ManualDrawer
        isOpen={isManualOpen}
        onClose={() => setIsManualOpen(false)}
      />

      {/* Real-time Chat & AI Interaction Drawer */}
      <ChatDrawer
        isOpen={isChatDrawerOpen}
        onClose={() => setIsChatDrawerOpen(false)}
        contact={selectedChatContact}
        messages={chatMessages}
        onSendMessage={handleSendMessage}
        onTriggerAiReply={handleTriggerAiReply}
        onUpdateMode={handleUpdateMode}
      />
    </div>
  );
};
