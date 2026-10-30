import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { ContactItemDto, GroupFileItemDto } from '@/api/contracts';
import type { AgentTask, TaskReport, TaskState } from '@/features/tasks/types';
import { NativeMessageText } from '@/components/chat/NativeFace';

const states: Record<TaskState, string> = { ready: '待执行', running: '执行中', completed: '已完成', failed: '失败', cancelled: '已取消', interrupted: '已中断' };
const fieldClass = 'w-full rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-2 text-sm text-slate-800 dark:text-slate-200 focus:border-sky-500 focus:outline-none disabled:opacity-50';
const buttonClass = 'rounded-lg border border-slate-200 dark:border-slate-700 px-3 py-2 text-xs text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 disabled:opacity-50 disabled:cursor-not-allowed';
const present = (value: unknown) => typeof value === 'string' ? value : JSON.stringify(value, null, 2);

export function TasksView({ accountUin, contacts, onDraftCreated }: { accountUin?: string; contacts: ContactItemDto[]; onDraftCreated?: () => void }) {
  const [tasks, setTasks] = useState<AgentTask[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [target, setTarget] = useState('');
  const [title, setTitle] = useState('');
  const [instruction, setInstruction] = useState('');
  const [persona, setPersona] = useState('');
  const [allowTodos, setAllowTodos] = useState(false);
  const [availableFiles, setAvailableFiles] = useState<GroupFileItemDto[]>([]);
  const [selectedFileIds, setSelectedFileIds] = useState<string[]>([]);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [followup, setFollowup] = useState('');
  const [busy, setBusy] = useState(false);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [exportingId, setExportingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const epoch = useRef(0);
  const mutationEpoch = useRef(0);
  const creating = useRef(false);
  const running = useRef<string | null>(null);
  const exporting = useRef(false);
  const currentAccount = useRef(accountUin);
  currentAccount.current = accountUin;
  const selected = tasks.find(task => task.id === selectedId);
  const contact = contacts.find(item => `${item.targetType}:${item.targetId}` === target);
  const isRunning = selected?.state === 'running' || selected?.id === runningId;

  useEffect(() => {
    let disposed = false;
    setAvailableFiles([]); setSelectedFileIds([]); setLoadingFiles(false);
    if (!accountUin || contact?.targetType !== 'group') return;
    setLoadingFiles(true);
    api.getGroupFiles(contact.targetId, undefined, true).then(response => {
      if (disposed) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '读取群文件缓存失败');
      setAvailableFiles(response.data.filter(file => file.downloadStatus === 'downloaded' && !file.isFolder));
    }).catch(reason => { if (!disposed) setError(reason instanceof Error ? reason.message : String(reason)); })
      .finally(() => { if (!disposed) setLoadingFiles(false); });
    return () => { disposed = true; };
  }, [accountUin, contact?.targetType, contact?.targetId]);

  const apply = useCallback((report: TaskReport, account: string, replace = false) => {
    if (currentAccount.current !== account) return;
    setTasks(previous => {
      const newer = (task: AgentTask) => {
        const saved = previous.find(item => item.id === task.id);
        return saved && (saved.revision > task.revision || saved.revision === task.revision && saved.updatedAt > task.updatedAt) ? saved : task;
      };
      const list = replace ? (report.tasks || []).filter(task => task.accountUin === account).map(newer) : previous;
      if (!report.task || report.task.accountUin !== account) return list;
      return [newer(report.task), ...list.filter(task => task.id !== report.task!.id)];
    });
    if (report.deletedId) setTasks(previous => previous.filter(task => task.id !== report.deletedId));
  }, []);

  useEffect(() => {
    const version = ++epoch.current;
    const mutation = ++mutationEpoch.current;
    creating.current = false; running.current = null; exporting.current = false;
    setTasks([]); setSelectedId(null); setTarget(''); setTitle(''); setInstruction(''); setPersona(''); setFollowup(''); setAllowTodos(false); setError(null); setNotice(null); setBusy(false); setRunningId(null); setExportingId(null);
    if (!accountUin) return;
    api.manageTasks({ operation: 'list' }).then(response => {
      if (version !== epoch.current || mutation !== mutationEpoch.current) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '读取任务失败');
      apply(response.data, accountUin, true);
    }).catch(reason => { if (version === epoch.current) setError(String(reason instanceof Error ? reason.message : reason)); });
    return () => { epoch.current++; };
  }, [accountUin, apply]);

  useEffect(() => {
    if (!accountUin || !tasks.some(task => task.state === 'running') && !runningId) return;
    let pending = false;
    let disposed = false;
    const timer = window.setInterval(async () => {
      if (pending) return;
      pending = true;
      const mutation = mutationEpoch.current;
      try {
        const response = await api.manageTasks({ operation: 'list' });
        if (!disposed && mutation === mutationEpoch.current && response.success && response.data) apply(response.data, accountUin, true);
      } catch { /* Polling retries while the original operation reports errors. */ }
      finally { pending = false; }
    }, 1500);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [accountUin, tasks, runningId, apply]);

  const request = async (operation: string, extra: Record<string, unknown> = {}) => {
    if (!accountUin) return;
    const version = epoch.current;
    const account = accountUin;
    mutationEpoch.current++;
    setError(null); setNotice(null);
    try {
      const response = await api.manageTasks({ operation, ...extra });
      if (version !== epoch.current) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '任务操作失败');
      mutationEpoch.current++;
      apply(response.data, account);
      return response.data;
    } catch (reason) {
      if (version !== epoch.current) return;
      setError(reason instanceof Error ? reason.message : String(reason));
      const refreshMutation = mutationEpoch.current;
      try {
        const refreshed = await api.manageTasks({ operation: 'list' });
        if (version === epoch.current && refreshMutation === mutationEpoch.current && refreshed.success && refreshed.data) apply(refreshed.data, account, true);
      } catch { /* Retain the original operation error if refreshing also fails. */ }
    }
  };

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if (creating.current || !contact || !title.trim() || !instruction.trim()) return;
    creating.current = true;
    const version = epoch.current;
    setBusy(true);
    const report = await request('create', { targetId: contact.targetId, targetType: contact.targetType === 'group' ? 'group' : 'private', title: title.trim(), instruction: instruction.trim(), persona: persona.trim(), allowTodos, selectedFileIds });
    if (version !== epoch.current) return;
    if (report?.task) { setSelectedId(report.task.id); setTitle(''); setInstruction(''); setPersona(''); }
    creating.current = false; setBusy(false);
  };

  const run = async () => {
    if (!selected || running.current || selected.state === 'running') return;
    const version = epoch.current;
    const id = selected.id;
    running.current = id;
    setRunningId(id);
    const report = await request('run', { taskId: id, expectedRevision: selected.revision, ...(followup.trim() ? { instruction: followup.trim() } : {}) });
    if (version === epoch.current) {
      running.current = null; setRunningId(null);
      if (report?.task?.state === 'completed') setFollowup('');
    }
  };

  const saveCandidate = async () => {
    if (!selected || selected.state !== 'completed' || exporting.current || !selected.candidateReply) return;
    const version = epoch.current;
    exporting.current = true; setExportingId(selected.id);
    const report = await request('draft', { taskId: selected.id, expectedRevision: selected.revision });
    if (version !== epoch.current) return;
    exporting.current = false; setExportingId(null);
    if (report?.draftId) { setNotice('已转入草稿审核'); onDraftCreated?.(); }
  };

  return <div className="flex h-full min-h-0 flex-col overflow-hidden p-6 dark:bg-slate-950">
    <header className="mb-4 flex items-center justify-between border-b border-slate-200 dark:border-slate-700 pb-4">
      <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">任务工作台</h2>
      <span className="text-xs text-slate-500 dark:text-slate-400">{accountUin || '未绑定账号'}</span>
    </header>
    {error && <p role="alert" className="mb-3 rounded-lg bg-red-50 dark:bg-red-950/40 p-3 text-sm text-red-800 dark:text-red-200">{error}</p>}
    {notice && <p role="status" className="mb-3 text-xs text-sky-700 dark:text-sky-300">{notice}</p>}
    <div className="grid min-h-0 flex-1 gap-5 overflow-y-auto lg:grid-cols-[280px_minmax(0,1fr)]">
      <div className="space-y-4">
        <form onSubmit={create} aria-label="新建任务" className="space-y-3 rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-4">
          <label className="block text-xs text-slate-600 dark:text-slate-300">会话<select aria-label="任务会话" value={target} onChange={event => setTarget(event.target.value)} required disabled={!accountUin || busy} className={`${fieldClass} mt-1`}>
            <option value="">选择联系人或群</option>
            {contacts.map(item => <option key={`${item.targetType}:${item.targetId}`} value={`${item.targetType}:${item.targetId}`}>{item.targetType === 'group' ? '群' : '好友'} · {item.name} ({item.targetId})</option>)}
          </select></label>
          <label className="block text-xs text-slate-600 dark:text-slate-300">标题<input aria-label="任务标题" value={title} onChange={event => setTitle(event.target.value)} maxLength={100} required className={`${fieldClass} mt-1`} disabled={!accountUin || busy} /></label>
          <label className="block text-xs text-slate-600 dark:text-slate-300">任务要求<textarea aria-label="任务要求" value={instruction} onChange={event => setInstruction(event.target.value)} maxLength={8000} required rows={4} className={`${fieldClass} mt-1 resize-y`} disabled={!accountUin || busy} /></label>
          <label className="block text-xs text-slate-600 dark:text-slate-300">本任务表达偏好<textarea aria-label="任务表达偏好" value={persona} onChange={event => setPersona(event.target.value)} maxLength={2000} rows={2} className={`${fieldClass} mt-1 resize-y`} disabled={!accountUin || busy} /></label>
          <label className="flex items-center gap-2 text-xs text-slate-600 dark:text-slate-300"><input type="checkbox" checked={allowTodos} onChange={event => setAllowTodos(event.target.checked)} disabled={!accountUin || busy} />允许写入本任务待办</label>
          {!!availableFiles.length && <fieldset className="space-y-2 text-xs text-slate-600 dark:text-slate-300"><legend className="mb-2">允许读取的本地群文件（最多 5 个）</legend>{availableFiles.map(file => <label key={file.fileId} className="flex items-start gap-2 break-all"><input type="checkbox" checked={selectedFileIds.includes(file.fileId)} disabled={busy || selectedFileIds.length >= 5 && !selectedFileIds.includes(file.fileId)} onChange={event => setSelectedFileIds(previous => event.target.checked ? [...previous, file.fileId] : previous.filter(id => id !== file.fileId))} />{file.fileName}</label>)}</fieldset>}
          <button type="submit" className="w-full rounded-lg bg-sky-600 px-3 py-2 text-sm text-white hover:bg-sky-700 disabled:opacity-50" disabled={!accountUin || busy || loadingFiles || !contact || !title.trim() || !instruction.trim()}>{busy ? '正在创建' : '创建任务'}</button>
        </form>
        <nav aria-label="任务列表" className="space-y-1">
          {tasks.map(task => <button type="button" key={task.id} onClick={() => { setSelectedId(task.id); setFollowup(''); }} aria-current={task.id === selectedId ? 'true' : undefined} className={`flex w-full items-start justify-between gap-2 rounded-lg border p-3 text-left ${task.id === selectedId ? 'border-sky-200 dark:border-sky-700 bg-sky-50 dark:bg-sky-950' : 'border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-slate-50 dark:hover:bg-slate-800'}`}>
            <span className={`min-w-0 break-words text-sm ${task.id === selectedId ? 'text-sky-950 dark:text-sky-100' : 'text-slate-800 dark:text-slate-200'}`}>{task.title}</span><span className="shrink-0 text-xs text-slate-500 dark:text-slate-400">{states[task.state]}</span>
          </button>)}
        </nav>
      </div>
      {selected && <section aria-label="任务详情" aria-busy={isRunning} className="min-w-0 space-y-4 rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3"><h3 className="font-semibold text-slate-900 dark:text-slate-100">{selected.title}</h3><span className="text-xs text-slate-500 dark:text-slate-400">{states[isRunning ? 'running' : selected.state]} · 版本 {selected.revision}</span></div>
        <p className="break-words text-xs text-slate-500 dark:text-slate-400">{selected.targetType === 'group' ? '群' : '好友'} {selected.targetId} · 独立会话 {selected.session.id}</p>
        <p className="whitespace-pre-wrap break-words text-sm text-slate-700 dark:text-slate-300">{selected.instruction}</p>
        {selected.error && <p role="alert" className="rounded-lg bg-red-50 dark:bg-red-950/40 p-3 text-sm text-red-800 dark:text-red-200">{selected.error}</p>}
        <div className="space-y-2">
          <label className="block text-xs text-slate-600 dark:text-slate-300">本次补充要求<textarea aria-label="本次补充要求" rows={2} value={followup} onChange={event => setFollowup(event.target.value)} disabled={isRunning} className={`${fieldClass} mt-1 resize-y`} /></label>
          <div className="flex flex-wrap gap-2">
            <button type="button" onClick={() => void run()} disabled={isRunning || !!runningId} className="rounded-lg bg-sky-600 px-4 py-2 text-xs text-white hover:bg-sky-700 disabled:opacity-50">{selected.messages.length ? '继续任务' : '执行任务'}</button>
            {isRunning && <button type="button" onClick={() => void request('cancel', { taskId: selected.id })} className={buttonClass}>取消执行</button>}
            {selected.state === 'completed' && selected.candidateReply != null && <button type="button" onClick={() => void saveCandidate()} disabled={isRunning || !!exportingId} className={buttonClass}>{exportingId === selected.id ? '正在转入' : '转入草稿'}</button>}
            <button type="button" onClick={() => void request('delete', { taskId: selected.id, expectedRevision: selected.revision }).then(report => { if (report) setSelectedId(null); })} disabled={isRunning} className={buttonClass}>删除任务</button>
          </div>
          {!!runningId && selected.id !== runningId && <p className="text-xs text-slate-500 dark:text-slate-400">另一个任务正在执行</p>}
        </div>
        {selected.activeRun && <p className="text-xs text-slate-500 dark:text-slate-400">模型调用 {selected.activeRun.modelCalls}/{selected.activeRun.maxModelCalls} · 限时 {selected.activeRun.maxDurationSeconds} 秒</p>}
        {selected.result && <div aria-label="任务结果" className="whitespace-pre-wrap break-words rounded-lg bg-slate-50 dark:bg-slate-800/60 p-4 text-sm text-slate-800 dark:text-slate-200"><NativeMessageText content={selected.result} /></div>}
        {!!selected.todos.length && <section aria-label="任务待办"><h4 className="mb-2 text-xs font-semibold text-slate-500 dark:text-slate-400">待办</h4><ul className="space-y-2">{selected.todos.map(todo => <li key={todo.id} className="break-words text-sm text-slate-700 dark:text-slate-300">{todo.done ? '已完成' : '待处理'} · {todo.text}</li>)}</ul></section>}
        {!!selected.steps.length && <section aria-label="执行步骤"><h4 className="mb-2 text-xs font-semibold text-slate-500 dark:text-slate-400">执行记录与来源</h4><div className="space-y-2">{selected.steps.map(step => <details key={step.id} className="rounded-lg border border-slate-200 dark:border-slate-700 p-3"><summary className="cursor-pointer text-xs text-slate-700 dark:text-slate-300">{step.kind} · {step.state}</summary><pre className="mt-2 whitespace-pre-wrap break-all text-xs text-slate-600 dark:text-slate-300">{present(step.output)}</pre>{!!step.sources?.length && <pre aria-label="任务来源" className="mt-2 whitespace-pre-wrap break-all text-xs text-slate-500 dark:text-slate-400">{present(step.sources)}</pre>}</details>)}</div></section>}
        {!!selected.messages.length && <details><summary className="cursor-pointer text-xs font-semibold text-slate-500 dark:text-slate-400">本任务对话</summary><div className="mt-2 space-y-3">{selected.messages.map((message, index) => <p key={index} className="whitespace-pre-wrap break-words text-xs text-slate-600 dark:text-slate-300"><span className="font-semibold">{message.role === 'user' ? '我' : message.role === 'assistant' ? '助手' : '工具'}：</span>{message.content}</p>)}</div></details>}
      </section>}
    </div>
  </div>;
}
