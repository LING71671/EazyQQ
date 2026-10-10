import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  ApiResponse,
  AccountReport,
  AccountInfoDto,
  BatchAccountOutcome,
  RepairReport,
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
  AppUpdateProgress,
  NapCatUpdateInfo,
  ModelInfoDto,
  FreeModelsReport,
  FreeModelProgress,
} from './contracts';

export const api = {
  detectFreeModels: (probe: boolean, requestId: string): Promise<ApiResponse<FreeModelsReport>> => invoke('detect_free_models', { probe, requestId }),
  onFreeModelProgress: (callback: (progress: FreeModelProgress) => void): Promise<UnlistenFn> => listen<FreeModelProgress>('free-model-progress', event => callback(event.payload)),
  getAccountStatus: (uin: string): Promise<ApiResponse<AccountReport>> => invoke('get_account_status', { uin }),
  listAccounts: (): Promise<ApiResponse<AccountReport[]>> => invoke('list_accounts'),
  registerAccount: (uin: string): Promise<ApiResponse<AccountInfoDto>> => invoke('register_account', { uin }),
  configureAccount: (uin: string, autoStart: boolean): Promise<ApiResponse<void>> => invoke('configure_account', { uin, autoStart }),
  batchAccounts: (operation: string, uins: string[]): Promise<ApiResponse<BatchAccountOutcome[]>> => invoke('batch_accounts', { operation, uins }),
  accountQrCode: (uin: string, refresh = false): Promise<ApiResponse<{ uin: string; qrcodeBase64?: string; loggedIn?: boolean }>> => invoke('account_qrcode', { uin, refresh }),
  repairChain: (): Promise<ApiResponse<RepairReport>> => invoke('repair_chain'),
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
  testAiConnection: async (
    options?: {
      provider?: string;
      modelId?: string;
      baseUrl?: string;
      apiKey?: string;
      prompt?: string;
    } | string,
    modelIdLegacy?: string
  ): Promise<ApiResponse<{
    isSuccess: boolean;
    latencyMs: number;
    provider?: string;
    model?: string;
    endpoint?: string;
    prompt?: string;
    reply?: string;
    reasoning?: string;
  }>> => {
    let payload = {};
    if (typeof options === 'string') {
      payload = { provider: options, modelId: modelIdLegacy };
    } else if (options) {
      payload = {
        provider: options.provider,
        modelId: options.modelId,
        baseUrl: options.baseUrl,
        apiKey: options.apiKey,
        prompt: options.prompt,
      };
    }
    return invoke('test_ai_connection', payload);
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
  onAppUpdateProgress: (callback: (progress: AppUpdateProgress) => void): Promise<UnlistenFn> => {
    return listen<AppUpdateProgress>('app-update-progress', event => callback(event.payload));
  },
  onNapCatUpdateProgress: (callback: (progress: AppUpdateProgress) => void): Promise<UnlistenFn> => {
    return listen<AppUpdateProgress>('napcat-update-progress', event => callback(event.payload));
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
  minimizeWindow: async (): Promise<boolean> => {
    return invoke('app_minimize_window');
  },
  toggleMaximizeWindow: async (): Promise<boolean> => {
    return invoke('app_toggle_maximize_window');
  },
  closeWindow: async (): Promise<boolean> => {
    return invoke('app_close_window');
  },
  startDragWindow: async (): Promise<void> => {
    return invoke('app_start_drag_window');
  },
  showWindow: async (): Promise<void> => {
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
