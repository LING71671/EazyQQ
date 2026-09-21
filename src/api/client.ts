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
  DependencyHealthReport,
  AppConfig
} from './contracts';

export const api = {
  // Protocol & Auth
  getProtocolStatus: async (): Promise<ApiResponse<ProtocolStatusDto>> => {
    return invoke('get_protocol_status');
  },
  refreshQrCode: async (): Promise<ApiResponse<{ qrcodeBase64: string; expiresInSeconds: number }>> => {
    return invoke('refresh_qrcode');
  },
  logout: async (): Promise<ApiResponse<void>> => {
    return invoke('logout');
  },

  // Contacts & Rules
  getContacts: async (params?: { type?: string; searchKeyword?: string }): Promise<ApiResponse<{ list: ContactItemDto[]; total: number }>> => {
    return invoke('get_contacts', { params });
  },
  updateRule: async (rule: Partial<RoutingRuleDto> & { targetId: string }): Promise<ApiResponse<RoutingRuleDto>> => {
    return invoke('update_rule', { rule });
  },
  batchUpdateMode: async (targetIds: string[], mode: string): Promise<ApiResponse<{ affectedCount: number }>> => {
    return invoke('batch_update_mode', { targetIds, mode });
  },

  // Messages & Drafts
  getMessages: async (targetId: string, limit = 30, offset = 0): Promise<ApiResponse<MessageItemDto[]>> => {
    return invoke('get_messages', { targetId, limit, offset });
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
  }
};
