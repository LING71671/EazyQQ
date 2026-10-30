import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { TasksView } from '../TasksView';
import type { AgentTask } from '@/features/tasks/types';
import type { ContactItemDto } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ manage: vi.fn(), files: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { manageTasks: mocks.manage, getGroupFiles: mocks.files } }));
const task = (changes: Partial<AgentTask> = {}): AgentTask => ({ id: 'task-1', accountUin: '10001', targetId: '20001', targetType: 'group', title: '整理讨论', instruction: '列出已经确定的安排', persona: '', allowTodos: false, revision: 3, state: 'ready', session: { id: 'session-1', generation: 1, provider: 'deepseek', model: 'test' }, messages: [], steps: [], todos: [], result: null, candidateReply: null, error: null, activeRun: null, createdAt: 1, updatedAt: 1, ...changes });
const contacts = [{ id: 'group:20001', targetType: 'group', targetId: '20001', name: '项目群' }] as ContactItemDto[];
const report = (item: AgentTask | null, tasks: AgentTask[] = item ? [item] : []) => ({ success: true, data: { task: item, tasks }, timestamp: 1 });
beforeEach(() => { mocks.manage.mockReset(); mocks.manage.mockResolvedValue(report(null)); mocks.files.mockReset().mockResolvedValue({ success: true, data: [] }); });
afterEach(cleanup);

it('creates a task for the selected typed conversation without starting inference', async () => {
  mocks.manage.mockResolvedValueOnce(report(null)).mockResolvedValueOnce(report(task()));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { fireEvent.change(screen.getByLabelText('任务会话'), { target: { value: 'group:20001' } }); });
  expect(mocks.files).toHaveBeenLastCalledWith('20001', undefined, true);
  fireEvent.change(screen.getByLabelText('任务标题'), { target: { value: '整理讨论' } });
  fireEvent.change(screen.getByLabelText('任务要求'), { target: { value: '列出已经确定的安排' } });
  fireEvent.click(screen.getByLabelText('允许写入本任务待办'));
  await act(async () => { fireEvent.submit(screen.getByRole('form', { name: '新建任务' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'create', targetId: '20001', targetType: 'group', title: '整理讨论', instruction: '列出已经确定的安排', persona: '', allowTodos: true, selectedFileIds: [] });
  expect(screen.getByRole('button', { name: '执行任务' })).toBeTruthy();
  expect(mocks.manage.mock.calls.some(([request]) => request.operation === 'run')).toBe(false);
});

it('runs with the visible revision and permits cancellation while inference is pending', async () => {
  let finish!: (value: unknown) => void;
  mocks.manage.mockResolvedValueOnce(report(task())).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValueOnce(report(task({ state: 'cancelled', revision: 4 })));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  fireEvent.change(screen.getByLabelText('本次补充要求'), { target: { value: '只保留今天的安排' } });
  fireEvent.click(screen.getByRole('button', { name: '执行任务' }));
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'run', taskId: 'task-1', expectedRevision: 3, instruction: '只保留今天的安排' });
  expect((screen.getByRole('button', { name: '删除任务' }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '取消执行' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'cancel', taskId: 'task-1' });
  await act(async () => { finish(report(task({ state: 'cancelled', revision: 4 }))); });
  expect(screen.getAllByText('已取消').length).toBeGreaterThan(0);
});

it('ignores an old account listing after the current account changes', async () => {
  let finish!: (value: unknown) => void;
  mocks.manage.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValueOnce(report(task({ id: 'task-2', accountUin: '10002', title: '另一个账号任务' })));
  const view = render(<TasksView accountUin="10001" contacts={contacts} />);
  view.rerender(<TasksView accountUin="10002" contacts={[]} />);
  await waitFor(() => expect(screen.getByRole('button', { name: /另一个账号任务/ })).toBeTruthy());
  await act(async () => { finish(report(task())); });
  expect(screen.queryByRole('button', { name: /整理讨论/ })).toBeNull();
});

it('shows tool output as plain text and exposes scoped sources', async () => {
  mocks.manage.mockResolvedValue(report(task({ state: 'completed', result: '安排已整理', steps: [{ id: 'step-1', runId: 'run-1', kind: 'history_search', state: 'completed', input: {}, output: '<script>untrusted</script>', sources: [{ kind: 'message', messageId: 'msg-1', senderUin: '30001' }], startedAt: 1 }] })));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  expect(screen.getByText('<script>untrusted</script>')).toBeTruthy();
  expect(screen.getByLabelText('任务来源').textContent).toContain('msg-1');
  expect(screen.getByLabelText('任务结果').textContent).toBe('安排已整理');
});

it('keeps existing tasks when a mutation response only contains its changed task', async () => {
  const other = task({ id: 'task-2', title: '另一个任务' });
  mocks.manage.mockResolvedValueOnce(report(null, [task(), other])).mockResolvedValueOnce(report(task({ state: 'completed', revision: 4 }), []));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '执行任务' })); });
  expect(screen.getByRole('button', { name: /另一个任务/ })).toBeTruthy();
});

it('exports a completed candidate for review without sending it', async () => {
  const completed = task({ state: 'completed', candidateReply: { schemaVersion: 1, bubbles: [] } });
  mocks.manage.mockResolvedValueOnce(report(completed)).mockResolvedValueOnce({ ...report(completed, []), data: { ...report(completed, []).data, draftId: 'task-draft-1' } });
  const refresh = vi.fn();
  render(<TasksView accountUin="10001" contacts={contacts} onDraftCreated={refresh} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '转入草稿' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'draft', taskId: 'task-1', expectedRevision: 3 });
  expect(refresh).toHaveBeenCalledOnce();
  expect(screen.getByRole('status').textContent).toBe('已转入草稿审核');
});

it('only offers already downloaded group files and records explicit file permission', async () => {
  mocks.files.mockResolvedValue({ success: true, data: [{ fileId: 'local-1', fileName: '已下载.txt', downloadStatus: 'downloaded', isFolder: false }, { fileId: 'remote-1', fileName: '未下载.txt', downloadStatus: 'remote', isFolder: false }] });
  mocks.manage.mockResolvedValueOnce(report(null)).mockResolvedValueOnce(report(task()));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { fireEvent.change(screen.getByLabelText('任务会话'), { target: { value: 'group:20001' } }); });
  expect(screen.queryByLabelText('未下载.txt')).toBeNull();
  fireEvent.click(screen.getByLabelText('已下载.txt'));
  fireEvent.change(screen.getByLabelText('任务标题'), { target: { value: '读取文件' } });
  fireEvent.change(screen.getByLabelText('任务要求'), { target: { value: '总结选择的文件' } });
  await act(async () => { fireEvent.submit(screen.getByRole('form', { name: '新建任务' })); });
  expect(mocks.manage.mock.calls.at(-1)?.[0].selectedFileIds).toEqual(['local-1']);
});

it('refreshes a stale revision after refusal while retaining the followup requirement', async () => {
  mocks.manage.mockResolvedValueOnce(report(task())).mockResolvedValueOnce({ success: false, error: { message: '任务版本已变化' } }).mockResolvedValueOnce(report(null, [task({ revision: 8 })]));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  fireEvent.change(screen.getByLabelText('本次补充要求'), { target: { value: '保留这次补充' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '执行任务' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'list' });
  expect(screen.getByRole('alert').textContent).toContain('任务版本已变化');
  expect(screen.getByText(/版本 8/)).toBeTruthy();
  expect((screen.getByLabelText('本次补充要求') as HTMLTextAreaElement).value).toBe('保留这次补充');
});

it('visibly disables another task execution while the current GUI run is pending', async () => {
  let finish!: (value: unknown) => void;
  mocks.manage.mockResolvedValueOnce(report(null, [task(), task({ id: 'task-2', title: '另一个任务' })])).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<TasksView accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: /整理讨论/ }));
  fireEvent.click(screen.getByRole('button', { name: '执行任务' }));
  fireEvent.click(screen.getByRole('button', { name: /另一个任务/ }));
  expect((screen.getByRole('button', { name: '执行任务' }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText('另一个任务正在执行')).toBeTruthy();
  await act(async () => { finish(report(task({ state: 'completed', revision: 4 }), [])); });
});
