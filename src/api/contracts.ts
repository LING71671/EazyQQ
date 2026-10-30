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
  /** Why no QR code could be obtained, when that is the case. */
  qrcodeError?: string;
  qqNumber?: string;
  nickname?: string;
  avatarUrl?: string;
  connectTime?: number;
  quickLoginAccounts?: QuickLoginAccountDto[];
}

export interface QuickLoginAccountDto {
  uin: string;
  nickname: string;
  faceUrl?: string;
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

export type RuleMode = 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';

export interface RoutingRuleDto {
  id: string;
  targetId: string;
  mode: RuleMode;
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

export interface SummaryChunkPayload {
  chunk: string;
}

export interface SummaryEndPayload {
  summary: GroupSummaryDto;
}

export interface DependencyHealthReport {
  isAllReady: boolean;
  qqNt: { ready: boolean; path: string; error?: string };
  openCode: { ready: boolean; path?: string; version?: string; activeModel?: string; error?: string };
  ports: { napcatPort: number; opencodePort: number; isConflict: boolean };
  storage: { workspacePath: string; isWritable: boolean; freeSpaceMb: number };
}

/**
 * Provider choices shared with the backend runtime configuration.
 * OpenCode uses its native executable; other providers use compatible HTTP endpoints.
 */
export type AiProviderId =
  | 'deepseek'
  | 'nvidia'
  | 'amd'
  | 'opencode'
  | 'ollama'
  | 'lmstudio'
  | 'llamacpp'
  | 'vllm'
  | 'openai';

export type SummaryIntervalType = '1h' | '2h' | '4h' | '6h' | '12h' | '24h' | 'custom';

export interface AppConfig {
  ai: {
    activeProvider: AiProviderId;
    model: string;
    temperature: number;
    maxContextMessages: number;
    baseUrl?: string;
    apiKey?: string;
    providers?: Record<string, { model: string; baseUrl?: string; apiKey?: string }>;
  };
  napcat: {
    wsPort: number;
    autoRestart: boolean;
    heartbeatIntervalSec: number;
  };
  storage: {
    autoSyncFiles: boolean;
    maxFileSizeMb: number;
  };
  summary: {
    enabled: boolean;
    intervalType: SummaryIntervalType;
    customIntervalMinutes: number;
    slidingWindowHours: number;
    autoForwardToPhone: boolean;
    customPrompt?: string;
  };
  window: {
    /** Collapse into the system tray instead of the taskbar when minimized. */
    minimizeToTray: boolean;
    /** Hide into the system tray instead of terminating the process on close. */
    closeToTray: boolean;
  };
}

export interface WindowBehaviorDto {
  minimizeToTray: boolean;
  closeToTray: boolean;
}

export interface AppUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseName: string;
  releaseNotes: string;
  htmlUrl: string;
  downloadUrl?: string;
  publishedAt: string;
  status: 'available' | 'up_to_date' | 'newer_local' | 'no_release' | 'installer_missing';
  installerName?: string | null;
  downloadSize?: number | null;
  checksumSha256?: string | null;
}

export interface AppUpdateProgress {
  operationId?: string;
  phase: 'checking' | 'downloading' | 'verifying' | 'installing' | 'ready';
  downloadedBytes: number;
  totalBytes: number;
}

export interface UpdateReceipt {
  schemaVersion: number;
  operationId: string;
  component: 'app' | 'napcat';
  targetVersion: string | null;
  phase: 'checking' | 'checked' | 'downloading' | 'verifying' | 'ready' | 'handoff' | 'waiting_parent' | 'installing' | 'installer_succeeded' | 'pending_verification' | 'deployed' | 'succeeded' | 'failed' | 'aborted';
  operation?: 'check' | 'install';
  verifiedVersion?: string | null;
  events?: { phase: UpdateReceipt['phase']; atMs: number }[];
  startedAtMs: number;
  updatedAtMs: number;
  installerExitCode: number | null;
  error: string | null;
}

export interface TelemetryStatus { enabled: boolean; configured: boolean; component: string; release: string }
export interface TelemetryTest { eventId: string; flushed: boolean; release: string }

export interface NapCatUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseName: string;
  releaseNotes: string;
  downloadUrl?: string;
  status: 'available' | 'up_to_date' | 'newer_local' | 'version_unknown' | 'asset_missing';
}

export interface ModelInfoDto {
  id: string;
  name: string;
  isFree: boolean;
  costInput?: number;
  costOutput?: number;
}

export interface FreeModelCheck { id: string; name: string; state: 'candidate' | 'available' | 'retired' | 'removed_from_catalogue' | 'unconfirmed' | 'requires_conditions'; detail: string; observedAtMs: number }
export interface FreeModelsHistory {
  requestId: string;
  status: 'completed' | 'cancelled' | 'failed';
  observedAtMs: number;
  models: FreeModelCheck[];
}
export interface FreeModelsReport {
  runtimeVersion: string;
  observedAtMs: number;
  catalogueIds: string[];
  models: FreeModelCheck[];
  credentialsUsed: boolean;
  source: string;
  requestId: string;
  status: 'completed' | 'cancelled' | 'failed';
  completed: number;
  total: number;
  catalogueRefreshed: boolean;
  error: string | null;
  cleanupError: string | null;
  history: FreeModelsHistory[];
}
export interface FreeModelProgress { requestId: string; completed: number; total: number; model: FreeModelCheck }

export interface AccountInfoDto { uin: string; nickname?: string | null; httpPort: number; wsPort: number; webuiPort: number; autoStart: boolean; processManaged: boolean }
export interface AccountReport {
  instance: AccountInfoDto;
  login: { loggedIn: boolean; uin?: string | null; nickname?: string | null; source: string };
  selected: boolean;
}
export interface BatchAccountOutcome { uin: string; ok: boolean; detail: string }
export interface RepairReport { action: string; attempted: boolean; ok: boolean; detail: string }
export type StyleSituation = 'acknowledgement' | 'question' | 'coordination' | 'explanation' | 'disagreement' | 'support' | 'repair' | 'casual';
export interface StyleProfile {
  methodology: string;
  sample: {
    messageCount: number;
    textMessageCount: number;
    turnCount: number;
    conversationCount: number;
    fromDate: string;
    toDate: string;
    holdoutTurnCount: number;
  };
  metrics: {
    textMedianChars: number;
    textP90Chars: number;
    bubblesPerTurnMedian: number;
    bubblesPerTurnP90: number;
    noTerminalPunctuationRate: number;
    questionOnlyRate: number;
    faceOnlyRate: number;
    mixedFaceRate: number;
    faceCoverageRate: number;
  };
  patterns: {
    id: string;
    situation: StyleSituation;
    observation: string;
    evidenceCount: number;
    conversationCount: number;
    confidence: 'low' | 'medium' | 'high';
  }[];
  demonstrations: {
    situation: StyleSituation;
    context: string;
    replies: string[];
    evidenceRefs: string[];
  }[];
  limitations: string[];
}
interface StylePackFields {
  id: string;
  name: string;
  version: string;
  rules: {
    sentenceLength: 'short' | 'balanced' | 'detailed';
    punctuation: 'minimal' | 'standard' | 'expressive';
    tone: 'casual' | 'neutral' | 'warm' | 'direct';
    lineBreaks: 'single' | 'natural' | 'frequent';
    preferredPhrases: string[];
    avoidPhrases: string[];
    qqFaceIds: string[];
  };
  statistics?: Record<string, number>;
}
export type StylePack = StylePackFields & (
  { schemaVersion: 1; profile?: never } | { schemaVersion: 2; profile: StyleProfile }
);

export interface StylePackStatus {
  enabled: boolean;
  activeId: string | null;
  packs: StylePack[];
  exportedPath?: string | null;
}
