export type TaskState = 'ready' | 'running' | 'completed' | 'failed' | 'cancelled' | 'interrupted';

export interface AgentTask {
  id: string;
  accountUin: string;
  targetId: string;
  targetType: 'private' | 'group';
  title: string;
  instruction: string;
  persona: string;
  allowTodos: boolean;
  selectedFileIds?: string[];
  revision: number;
  state: TaskState;
  session: { id: string; generation: number; provider: string; model: string };
  messages: Array<{ role: string; content: string }>;
  steps: Array<{ id: string; runId: string; kind: string; state: string; input: unknown; output: unknown; sources: unknown[]; startedAt: number; finishedAt?: number | null }>;
  todos: Array<{ id: string; text: string; done: boolean }>;
  result: string | null;
  candidateReply: unknown;
  error: string | null;
  activeRun: { id: string; sequence: number; state: string; inputRevision: number; startedAt: number; finishedAt?: number | null; modelCalls: number; maxModelCalls: number; maxDurationSeconds: number } | null;
  createdAt: number;
  updatedAt: number;
}

export interface TaskReport {
  tasks: AgentTask[];
  task: AgentTask | null;
  deletedId?: string;
  draftId?: string;
}
