import React, { useState, useEffect, useCallback } from 'react';
import { Sidebar, NavView } from '@/components/layout/Sidebar';
import { TopHeader } from '@/components/layout/TopHeader';
import { ManualDrawer } from '@/components/manual/ManualDrawer';
import { ChainHealthDrawer } from '@/components/health/ChainHealthDrawer';
import { BootSplash } from '@/components/common/BootSplash';
import { LoginView } from '@/views/LoginView';
import { ContactsView } from '@/views/ContactsView';
import { DraftsView } from '@/views/DraftsView';
import { FilesView } from '@/views/FilesView';
import { SummariesView } from '@/views/SummariesView';
import { SettingsView } from '@/views/SettingsView';
import { ChatDrawer } from '@/components/chat/ChatDrawer';
import { api } from '@/api/client';
import type { 
  GroupSummaryDto, 
  AppConfig, 
  DependencyHealthReport 
} from '@/api/contracts';
import { useProtocolState } from '@/hooks/useProtocolState';
import { useChatManager } from '@/hooks/useChatManager';
import { useGroupFilesManager } from '@/hooks/useGroupFilesManager';
import { useMotionPolicy } from '@/hooks/useMotionPolicy';

export const App: React.FC = () => {
  const reducedMotion = useMotionPolicy();
  const [currentView, setCurrentView] = useState<NavView>('login');
  const [isManualOpen, setIsManualOpen] = useState(false);
  const [isHealthOpen, setIsHealthOpen] = useState(false);
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const openManual = (event: KeyboardEvent) => {
      if (event.key === 'F1') {
        event.preventDefault();
        setIsManualOpen(previous => !previous);
      }
    };
    window.addEventListener('keydown', openManual);
    return () => window.removeEventListener('keydown', openManual);
  }, []);

  // Summaries & Diagnostics state
  const [summaries, setSummaries] = useState<GroupSummaryDto[]>([]);
  const [isSummarizing, setIsSummarizing] = useState(false);
  const [diagnosticsPath, setDiagnosticsPath] = useState<string | undefined>(undefined);

  // App Configuration
  const [config, setConfig] = useState<AppConfig>({
    ai: {
      activeProvider: 'opencode',
      model: '',
      baseUrl: 'https://opencode.ai/zen/v1',
      temperature: 0.7,
      maxContextMessages: 10,
    },
    napcat: {
      wsPort: 3001,
      autoRestart: true,
      heartbeatIntervalSec: 15,
    },
    storage: {
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
    window: {
      minimizeToTray: false,
      closeToTray: true,
    },
  });

  const [health, setHealth] = useState<DependencyHealthReport>({
    isAllReady: false,
    qqNt: { ready: false, path: '' },
    openCode: { ready: false, version: '', activeModel: '' },
    ports: { napcatPort: 3001, opencodePort: 4096, isConflict: false },
    storage: { workspacePath: '', isWritable: true, freeSpaceMb: 0 },
  });

  // Boot splash state
  const [isBooting, setIsBooting] = useState(true);
  const [bootStage, setBootStage] = useState('正在初始化本地运行环境');

  // Domain Hook 1: Chat, Contacts, Realtime messaging & Drafts
  const {
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
  } = useChatManager();

  // Domain Hook 2: Protocol status, login, QR code & chain health
  const handleLoginSuccess = useCallback(() => {
    fetchContactsAndDrafts();
    api.getSummaryHistory().then((sRes) => {
      if (sRes.success && sRes.data) setSummaries(sRes.data);
    });
  }, [fetchContactsAndDrafts]);

  const {
    pendingLogin,
    setPendingLogin,
    protocolStatus,
    setProtocolStatus,
    isRefreshingQr,
    qrError,
    isQuickLoggingIn,
    chainHasFailure,
    handleRefreshQr,
    handleQuickLogin,
    handleLogout,
  } = useProtocolState({ onLoginSuccess: handleLoginSuccess });

  // Domain Hook 3: Group Files & AI Summarization
  const {
    selectedGroupId,
    setSelectedGroupId,
    files,
    setFiles,
    isSyncingFiles,
    filesError,
    activeFileSummary,
    isSummarizingFile,
    handleSyncFiles,
    handleSelectGroup,
    handleDownloadFile,
    handleSummarizeFile,
    handleOpenFolder,
  } = useGroupFilesManager();

  // Initial boot data fetch
  useEffect(() => {
    api.showWindow().catch(() => {});

    const fetchInitialData = async () => {
      setBootStage('正在同步本地配置与联系人');

      try {
        const [contactsRes, summariesRes, configRes, draftsRes, healthRes] = await Promise.allSettled([
          api.getContacts(),
          api.getSummaryHistory(),
          api.getConfig(),
          api.getPendingDrafts(),
          api.checkDependencies(),
        ]);

        if (contactsRes.status === 'fulfilled' && contactsRes.value.success && contactsRes.value.data) {
          const list = contactsRes.value.data.list;
          setContacts(list);

          // Auto-select and preload group files
          const groups = list.filter((c) => c.targetType === 'group');
          const savedGid = localStorage.getItem('eazyqq_selected_group_id');
          const matchedGroup = (savedGid && groups.find((g) => g.targetId === savedGid)) || groups[0];
          if (matchedGroup) {
            setSelectedGroupId(matchedGroup.targetId);
            api.getGroupFiles(matchedGroup.targetId).then((fRes) => {
              if (fRes.success && fRes.data) {
                setFiles(fRes.data);
              }
            });
          }
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
        if (healthRes.status === 'fulfilled' && healthRes.value.success && healthRes.value.data) {
          setHealth(healthRes.value.data);
        }
      } catch (e) {
        console.error('Failed to load initial data from SQLite', e);
      } finally {
        setIsBooting(false);
      }
    };

    fetchInitialData();
  }, [setContacts, setDrafts, setFiles, setProtocolStatus, setSelectedGroupId]);

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

  const handleUpdateConfig = async (cfg: Partial<AppConfig>) => {
    const updated = { ...config, ...cfg };
    setConfig(updated);
    try {
      await api.updateConfig(updated);
    } catch (e) {
      console.error('Failed to update config in SQLite', e);
    }
  };

  const handleCheckHealth = async () => {
    try {
      const res = await api.checkDependencies();
      if (res.success && res.data) {
        setHealth(res.data);
      }
    } catch (e) {
      console.error('Failed to check dependencies', e);
    }
  };

  const handleExportDiagnostics = async () => {
    try {
      const res = await api.exportDiagnosticsBundle();
      if (res.success && res.data) {
        setDiagnosticsPath(res.data.zipFilePath);
      }
    } catch (e) {
      console.error('Failed to export diagnostics bundle', e);
    }
  };

  const handleGenerateSummary = async (targetId: string, hours: number) => {
    setIsSummarizing(true);
    try {
      const res = await api.generateSummaryStream(targetId, hours);
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
          onOpenHealth={() => setIsHealthOpen(true)}
          chainHasFailure={chainHasFailure}
        />

        {/* View Switcher */}
        <main key={currentView} className="motion-view flex-1 overflow-hidden bg-slate-50/50">
          {currentView === 'login' && (
            <LoginView
              status={protocolStatus}
              onRefreshQr={handleRefreshQr}
              isLoading={isRefreshingQr}
              error={qrError || protocolStatus.qrcodeError}
              onQuickLogin={handleQuickLogin}
              isQuickLoggingIn={isQuickLoggingIn}
              pendingLogin={pendingLogin}
              onCancelPendingLogin={() => setPendingLogin(null)}
              onOpenHealth={() => setIsHealthOpen(true)}
              onLogout={handleLogout}
            />
          )}
          {currentView === 'contacts' && (
            <ContactsView
              contacts={contacts}
              onUpdateMode={handleUpdateMode}
              onToggleSummaryWhitelist={handleToggleSummaryWhitelist}
              onUpdateTrigger={handleUpdateTrigger}
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
              groups={contacts.filter((c) => c.targetType === 'group')}
              selectedGroupId={selectedGroupId}
              onSelectGroup={handleSelectGroup}
              onSyncFiles={handleSyncFiles}
              isSyncing={isSyncingFiles}
              onDownloadFile={handleDownloadFile}
              onSummarizeFile={handleSummarizeFile}
              onOpenFolder={handleOpenFolder}
              activeSummary={activeFileSummary}
              isSummarizing={isSummarizingFile}
              errorMessage={filesError}
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
              onCheckHealth={handleCheckHealth}
              onExportDiagnostics={handleExportDiagnostics}
              diagnosticsPath={diagnosticsPath}
            />
          )}
        </main>
      </div>

      {/* F1 Built-in Manual Drawer */}
      <ManualDrawer
        isOpen={isManualOpen}
        onClose={() => setIsManualOpen(false)}
      />

      {/* Global Chain Health & Self-Healing Drawer */}
      <ChainHealthDrawer
        isOpen={isHealthOpen}
        onClose={() => setIsHealthOpen(false)}
        onOpenSettings={() => setCurrentView('settings')}
      />

      {/* Real-time Chat & AI Interaction Drawer */}
      <ChatDrawer
        isOpen={isChatDrawerOpen}
        onClose={handleCloseChat}
        contact={selectedChatContact}
        messages={chatMessages}
        onSendMessage={handleSendMessage}
        onTriggerAiReply={handleTriggerAiReply}
        onUpdateMode={handleUpdateMode}
      />

      {/* Boot overlay: animates until the first local data round-trip settles */}
      <BootSplash visible={isBooting} stageLabel={bootStage} reducedMotion={reducedMotion} />
    </div>
  );
};
