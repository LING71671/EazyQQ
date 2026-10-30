import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DraftsView } from '../DraftsView';
import type { PendingDraftDto, ReplyDeliveryReport } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ batches: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { manageReplyBatches: mocks.batches } }));
const draft = { id: 'draft-1', targetId: '20001', targetName: '项目群', targetType: 'group', generatedContent: '第一句\n第二行\n\n[CQ:face,id=427]', incomingMessageSnippet: '怎么样', modelUsed: 'test', replyPlan: { schemaVersion: 1, bubbles: [{ id: 'b1', segments: [{ type: 'text', text: '第一句\n第二行' }] }, { id: 'b2', segments: [{ type: 'face', faceKey: '427' }] }] } } as PendingDraftDto;
beforeEach(() => { mocks.batches.mockReset(); mocks.batches.mockResolvedValue({ success: false }); });
afterEach(cleanup);
const actions = () => ({ onSendDraft: vi.fn(), onDismissDraft: vi.fn(), onRegenerateDraft: vi.fn() });

it('preserves explicit bubbles, internal newlines and face-only messages through approval', async () => {
  const callbacks = actions();
  render(<DraftsView drafts={[draft]} {...callbacks} />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getAllByRole('textbox')).toHaveLength(2);
  expect(screen.getByLabelText('QQ表情发送预览').querySelector('img')?.alt).toBe('[偷感]');
  fireEvent.change(screen.getByLabelText('回复正文：项目群'), { target: { value: '改过的第一句\n保留换行' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '确认发送' })); });
  expect(callbacks.onSendDraft).toHaveBeenCalledWith('draft-1', '改过的第一句\n保留换行\n\n[CQ:face,id=427]', ['改过的第一句\n保留换行', '[CQ:face,id=427]']);
});

it('supports adding and deleting bubbles without splitting ordinary text', async () => {
  render(<DraftsView drafts={[{ ...draft, replyPlan: null, generatedContent: '一行\n两行' }]} {...actions()} />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getAllByRole('textbox')).toHaveLength(1);
  fireEvent.click(screen.getByRole('button', { name: '添加一条消息' }));
  expect(screen.getAllByRole('textbox')).toHaveLength(2);
  expect((screen.getByRole('button', { name: '确认发送' }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole('button', { name: '删除第2条消息' }));
  expect(screen.getAllByRole('textbox')).toHaveLength(1);
});

it('blocks editing and resending unknown receipts until explicit reconciliation', async () => {
  const delivery: ReplyDeliveryReport = { batchId: 'draft-1', status: 'unknown', messages: [], receipts: [{ bubbleId: 'b1', state: 'sent', messageId: 'm1' }, { bubbleId: 'b2', state: 'unknown', error: '超时' }] };
  mocks.batches.mockResolvedValueOnce({ success: true, data: delivery }).mockResolvedValue({ success: true, data: { ...delivery, status: 'partial', receipts: [{ bubbleId: 'b1', state: 'sent' }, { bubbleId: 'b2', state: 'pending' }] } });
  render(<DraftsView drafts={[draft]} {...actions()} />);
  await act(async () => { await Promise.resolve(); });
  expect((screen.getByLabelText('回复正文：项目群') as HTMLTextAreaElement).disabled).toBe(true);
  expect((screen.getByRole('button', { name: '继续发送未送达消息' }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '已核对未送达' })); });
  expect(mocks.batches).toHaveBeenCalledWith({ operation: 'reconcile', batchId: 'draft-1', bubbleId: 'b2', confirmedNotDelivered: true });
  expect((screen.getByRole('button', { name: '继续发送未送达消息' }) as HTMLButtonElement).disabled).toBe(false);
});

it('disables all draft actions until the approval operation settles', async () => {
  let finish!: () => void;
  const callbacks = actions();
  callbacks.onSendDraft.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
  render(<DraftsView drafts={[draft]} {...callbacks} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.click(screen.getByRole('button', { name: '确认发送' }));
  expect((screen.getByRole('button', { name: '处理中' }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole('button', { name: '重新生成' }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { finish(); });
});

it('lets the owner dismiss an obsolete batch while blocking its transmission', async () => {
  const callbacks = actions();
  mocks.batches.mockResolvedValue({ success: true, data: { batchId: 'draft-1', status: 'superseded', receipts: [{ bubbleId: 'b1', state: 'superseded' }], messages: [] } });
  render(<DraftsView drafts={[draft]} {...callbacks} />);
  await act(async () => { await Promise.resolve(); });
  expect((screen.getByRole('button', { name: '继续发送未送达消息' }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '驳回丢弃' })); });
  expect(callbacks.onDismissDraft).toHaveBeenCalledWith('draft-1');
});

it('directs derived draft regeneration back to its original bounded task', async () => {
  render(<DraftsView drafts={[{ ...draft, replyToMsgId: 'task:task-1:run-1' }]} {...actions()} />);
  await act(async () => { await Promise.resolve(); });
  expect((screen.getByRole('button', { name: '重新生成' }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText('任务草稿需要重新生成时，请在任务工作台继续修订。')).toBeTruthy();
  expect((screen.getByRole('button', { name: '确认发送' }) as HTMLButtonElement).disabled).toBe(false);
});
