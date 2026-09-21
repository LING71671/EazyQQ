// Data Transfer Objects matching API_SPECIFICATION.md

export interface ApiResponse<T = void> {
  success: boolean;
  data?: T;
  error?: ApiErrorPayload;
  timestamp: number;
}

export interface ApiErrorPayload {
  code: number;
  message: string;
  details?: Record<string, unknown>;
  suggestedAction?: string;
}

export interface ProtocolStatusDto {
  isConnected: boolean;
  loginStatus: 'unlogged' | 'waiting_scan' | 'scanned' | 'logged_in';
  qrcodeBase64?: string;
  qqNumber?: string;
  nickname?: string;
  avatarUrl?: string;
  connectTime?: number;
}

export interface ContactItemDto {
  id: string; // 'friend:10001' | 'group:20002'
  targetType: 'friend' | 'group';
  targetId: string;
  name: string;
  avatarUrl: string;
  remark?: string;
  memberCount?: number;
  rule: RoutingRuleDto;
  unreadCount: number;
  lastMessageSnippet?: string;
  lastMessageTimestamp?: number;
}

export interface RoutingRuleDto {
  id: string;
  targetId: string;
  mode: 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';
  triggerCondition: 'all' | 'at_me' | 'keyword';
  keywords: string[];
  systemPrompt?: string;
  modelId?: string;
  cooldownSeconds: number;
  enabled: boolean;
  isSummaryWhitelist?: boolean;
  summaryIntervalHours?: number;
}

export interface MessageItemDto {
  id: string;
  targetId: string;
  senderId: string;
  senderName: string;
  senderAvatar?: string;
  content: string;
  isFromMe: boolean;
  aiReplyStatus: 'none' | 'auto_replied' | 'draft_pending' | 'summarized';
  timestamp: number;
}

export interface PendingDraftDto {
  id: string;
  targetId: string;
  targetName: string;
  targetType: 'friend' | 'group';
  replyToMsgId: string;
  incomingMessageSnippet: string;
  generatedContent: string;
  thinkingContent?: string;
  modelUsed: string;
  confidenceScore?: number;
  createdAt: number;
}

export interface GroupFileItemDto {
  fileId: string;
  fileName: string;
  fileSize: number;
  fileUrl?: string;
  uploaderId: string;
  uploaderName: string;
  uploadTime: number;
  downloadStatus: 'remote' | 'downloading' | 'downloaded';
  localPath?: string;
  isFolder: boolean;
  folderId?: string;
}

export interface FileSummaryResultDto {
  fileName: string;
  fileSize: number;
  totalChars: number;
  summaryText: string;
  keyTakeaways: string[];
  actionItems: string[];
}

export interface GroupSummaryDto {
  id: string;
  targetId: string;
  targetName: string;
  summaryText: string;
  keyPoints: string[];
  decisions: string[];
  sharedFiles: string[];
  startTime: number;
  endTime: number;
  createdAt: number;
}

export interface DependencyHealthReport {
  isAllReady: boolean;
  qqNt: { ready: boolean; path: string; error?: string };
  openCode: { ready: boolean; version?: string; activeModel?: string; error?: string };
  ports: { napcatPort: number; opencodePort: number; isConflict: boolean };
  storage: { workspacePath: string; isWritable: boolean; freeSpaceMb: number };
}

export interface AppConfig {
  ai: {
    activeProvider: 'opencode' | 'openai' | 'deepseek';
    model: string;
    temperature: number;
    maxContextMessages: number;
    baseUrl?: string;
    apiKey?: string;
  };
  napcat: {
    wsPort: number;
    autoRestart: boolean;
    heartbeatIntervalSec: number;
  };
  storage: {
    workspaceDir: string;
    autoSyncFiles: boolean;
    maxFileSizeMb: number;
  };
  summary: {
    enabled: boolean;
    intervalType: '1h' | '2h' | '4h' | '6h' | '12h' | '24h' | 'custom';
    customIntervalMinutes: number;
    slidingWindowHours: number;
    autoForwardToPhone: boolean;
    customPrompt?: string;
  };
}
