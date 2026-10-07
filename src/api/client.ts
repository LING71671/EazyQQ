import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  ApiResponse,
  ProtocolStatusDto,
  ContactItemDto,
  RoutingRuleDto,
  MessageItemDto,
  PendingDraftDto,
  GroupFileItemDto,
  FileSummaryResultDto,
  GroupSummaryDto,
  SummaryChunkPayload,
  SummaryEndPayload,
  DependencyHealthReport,
  AppConfig,
  WindowBehaviorDto,
  QuickLoginAccountDto,
  AppUpdateInfo,
  NapCatUpdateInfo,
  ModelInfoDto,
} from './contracts';

export const api = {
  // Protocol & Auth
  getProtocolStatus: async (): Promise<ApiResponse<ProtocolStatusDto>> => {
    return invoke('get_protocol_status');
  },
  refreshQrCode: async (): Promise<ApiResponse<{ qrcodeBase64: string; expiresInSeconds: number }>> => {
    return invoke('refresh_qrcode');
  },
  quickLogin: async (uin: string): Promise<ApiResponse<void>> => {
    return invoke('quick_login', { uin });
  },
  getQuickLoginAccounts: async (): Promise<ApiResponse<QuickLoginAccountDto[]>> => {
    return invoke('get_quick_login_accounts');
  },
  logout: async (): Promise<ApiResponse<void>> => {
    return invoke('logout');
  },

  // Contacts & Rules
  getContacts: async (params?: { type?: string; searchKeyword?: string }): Promise<ApiResponse<{ list: ContactItemDto[]; total: number }>> => {
    return invoke('get_contacts', { params });
  },
  /**
   * End-to-end link report.
   *
   * `firstBreak` is the useful part: in a pipeline, everything after the first failure is
   * a consequence, so fixing anything else is wasted effort.
   */
  getChainStatus: async (): Promise<
    ApiResponse<{
      links: Array<{
        link: string;
        label: string;
        impact: string;
        health: 'ok' | 'unknown' | 'failed';
        detail: string;
        last_ok_secs_ago: number | null;
        last_error_secs_ago: number | null;
      }>;
      firstBreak: {
        link: string;
        label: string;
        impact: string;
        detail: string;
      } | null;
      hasFailure: boolean;
      uptimeSecs: number;
    }>
  > => {
    return invoke('get_chain_status');
  },
  restartNapCat: async (): Promise<ApiResponse<{ attempted: boolean; ok: boolean; detail: string }>> => {
    return invoke('restart_napcat');
  },
  /** Clear a conversation's unread badge. `at` defaults to now on the backend. */
  markRead: async (targetId: string, at?: number): Promise<ApiResponse<number>> => {
    return invoke('mark_read', { targetId, at });
  },
  updateRule: async (rule: Partial<RoutingRuleDto> & { targetId: string }): Promise<ApiResponse<RoutingRuleDto>> => {
    return invoke('update_rule', { rule });
  },
  batchUpdateMode: async (targetIds: string[], mode: string): Promise<ApiResponse<{ affectedCount: number }>> => {
    return invoke('batch_update_mode', { targetIds, mode });
  },

  // Messages & Drafts
  getMessages: async (targetId: string, limit = 30, offset = 0, targetType?: string): Promise<ApiResponse<MessageItemDto[]>> => {
    return invoke('get_messages', { targetId, limit, offset, targetType });
  },
  sendMessage: async (targetId: string, content: string): Promise<ApiResponse<{ messageId: string }>> => {
    return invoke('send_message', { targetId, content });
  },
  getPendingDrafts: async (): Promise<ApiResponse<PendingDraftDto[]>> => {
    return invoke('get_pending_drafts');
  },
  sendDraft: async (draftId: string, finalContent?: string): Promise<ApiResponse<{ sentMessageId: string }>> => {
    return invoke('send_draft', { draftId, finalContent });
  },
  dismissDraft: async (draftId: string): Promise<ApiResponse<void>> => {
    return invoke('dismiss_draft', { draftId });
  },
  regenerateDraft: async (draftId: string, customInstruction?: string): Promise<ApiResponse<PendingDraftDto>> => {
    return invoke('regenerate_draft', { draftId, customInstruction });
  },

  // Group Files
  getGroupFiles: async (groupId: string, folderId?: string): Promise<ApiResponse<GroupFileItemDto[]>> => {
    return invoke('get_group_files', { groupId, folderId });
  },
  downloadFile: async (groupId: string, fileId: string, fileName: string): Promise<ApiResponse<{ taskId: string; localSavePath: string }>> => {
    return invoke('download_file', { groupId, fileId, fileName });
  },
  summarizeFile: async (localFilePath: string): Promise<ApiResponse<FileSummaryResultDto>> => {
    return invoke('summarize_file', { localFilePath });
  },
  openFolder: async (targetPath: string): Promise<ApiResponse<void>> => {
    return invoke('open_folder', { targetPath });
  },

  // Summaries
  generateSummary: async (targetId: string, slidingWindowHours = 6): Promise<ApiResponse<GroupSummaryDto>> => {
    return invoke('generate_summary', { targetId, slidingWindowHours });
  },
  generateSummaryStream: async (targetId: string, slidingWindowHours = 6): Promise<ApiResponse<GroupSummaryDto>> => {
    return invoke('generate_summary_stream', { targetId, slidingWindowHours });
  },
  getSummaryHistory: async (targetId?: string): Promise<ApiResponse<GroupSummaryDto[]>> => {
    return invoke('get_summary_history', { targetId });
  },
  deleteSummary: async (id: string): Promise<ApiResponse<void>> => {
    return invoke('delete_summary', { id });
  },

  // Config & Diagnostics
  getConfig: async (): Promise<ApiResponse<AppConfig>> => {
    return invoke('get_config');
  },
  updateConfig: async (config: Partial<AppConfig>): Promise<ApiResponse<AppConfig>> => {
    return invoke('update_config', { config });
  },
  testAiConnection: async (provider: string, modelId?: string): Promise<ApiResponse<{ isSuccess: boolean; latencyMs: number }>> => {
    return invoke('test_ai_connection', { provider, modelId });
  },
  fetchProviderModels: async (provider: string, baseUrl?: string, apiKey?: string): Promise<ApiResponse<ModelInfoDto[]>> => {
    return invoke('fetch_provider_models', { provider, baseUrl, apiKey });
  },
  checkDependencies: async (): Promise<ApiResponse<DependencyHealthReport>> => {
    return invoke('check_dependencies');
  },
  exportDiagnosticsBundle: async (): Promise<ApiResponse<{ zipFilePath: string }>> => {
    return invoke('export_diagnostics_bundle');
  },

  triggerAiReply: async (targetId: string, contextSnippet: string): Promise<ApiResponse<string>> => {
    return invoke('trigger_ai_reply', { targetId, contextSnippet });
  },

  // Event Listeners
  onProtocolStatusChanged: (callback: (status: ProtocolStatusDto) => void): Promise<UnlistenFn> => {
    return listen<ProtocolStatusDto>('event:protocol-status-changed', (event) => callback(event.payload));
  },
  onDraftCreated: (callback: (draft: PendingDraftDto) => void): Promise<UnlistenFn> => {
    return listen<PendingDraftDto>('new-draft', (event) => callback(event.payload));
  },
  onMessageReceived: (callback: (msg: MessageItemDto) => void): Promise<UnlistenFn> => {
    return listen<MessageItemDto>('new-chat-message', (event) => callback(event.payload));
  },
  onSummaryChunk: (callback: (payload: SummaryChunkPayload) => void): Promise<UnlistenFn> => {
    return listen<SummaryChunkPayload>('summary-chunk', (event) => callback(event.payload));
  },
  onSummaryEnd: (callback: (payload: SummaryEndPayload) => void): Promise<UnlistenFn> => {
    return listen<SummaryEndPayload>('summary-end', (event) => callback(event.payload));
  },

  // Window Management
  // Routed through native Rust commands on purpose: custom commands are not gated by
  // the capability ACL, so window control keeps working even if a capability file is
  // missing or stale (that regression is what broke minimize / drag before).
  minimizeWindow: async (): Promise<ApiResponse<boolean>> => {
    return invoke('app_minimize_window');
  },
  toggleMaximizeWindow: async (): Promise<ApiResponse<boolean>> => {
    return invoke('app_toggle_maximize_window');
  },
  closeWindow: async (): Promise<ApiResponse<boolean>> => {
    return invoke('app_close_window');
  },
  startDragWindow: async (): Promise<ApiResponse<void>> => {
    return invoke('app_start_drag_window');
  },
  showWindow: async (): Promise<ApiResponse<void>> => {
    return invoke('app_show_window');
  },
  getWindowBehavior: async (): Promise<ApiResponse<WindowBehaviorDto>> => {
    return invoke('app_get_window_behavior');
  },
  // Remote Updater
  checkAppUpdate: async (): Promise<ApiResponse<AppUpdateInfo>> => {
    return invoke('check_app_update');
  },
  upgradeApp: async (downloadUrl?: string): Promise<ApiResponse<string>> => {
    return invoke('upgrade_app', { downloadUrl });
  },
  getQqPath: async (): Promise<ApiResponse<string>> => {
    return invoke('get_qq_path');
  },
  setQqPath: async (path: string): Promise<ApiResponse<string>> => {
    return invoke('set_qq_path', { path });
  },
  getNapCatVersion: async (): Promise<ApiResponse<string>> => {
    return invoke('get_napcat_version');
  },
  checkNapCatUpdate: async (): Promise<ApiResponse<NapCatUpdateInfo>> => {
    return invoke('check_napcat_update');
  },
  upgradeNapCat: async (downloadUrl?: string): Promise<ApiResponse<string>> => {
    return invoke('upgrade_napcat', { downloadUrl });
  },
};
