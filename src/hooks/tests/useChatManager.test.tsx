import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useChatManager } from '../useChatManager';
import { api } from '@/api/client';
import type { ContactItemDto, MessageItemDto, PendingDraftDto, RoutingRuleDto } from '@/api/contracts';

vi.mock('@/api/client', () => ({ api: {
  onDraftCreated: vi.fn(), onMessageReceived: vi.fn(), sendMessage: vi.fn(), triggerAiReply: vi.fn(),
  updateRule: vi.fn(), sendDraft: vi.fn(), dismissDraft: vi.fn(), regenerateDraft: vi.fn(),
  getMessages: vi.fn(), markRead: vi.fn(),
} }));

const contact = { id: 'group-10001', targetId: '10001', targetType: 'group', name: 'Group', unreadCount: 0,
  rule: { targetId: '10001', mode: 'ignore', enabled: false, isSummaryWhitelist: false } } as ContactItemDto;
const draft = { id: 'draft-1', targetId: '10001', generatedContent: 'Original draft' } as PendingDraftDto;
const failure = { success: false, timestamp: 0, error: { code: 1001, message: 'Storage unavailable' } };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.onDraftCreated).mockResolvedValue(vi.fn());
  vi.mocked(api.onMessageReceived).mockResolvedValue(vi.fn());
  vi.mocked(api.getMessages).mockResolvedValue({ success: true, timestamp: 0, data: [] });
  vi.mocked(api.markRead).mockResolvedValue({ success: true, timestamp: 0, data: 1 });
});
afterEach(cleanup);

it('passes the conversation type to manual sending', async () => {
  vi.mocked(api.sendMessage).mockResolvedValue({ success: true, timestamp: 0, data: { messageId: 'sent-1' } });
  const { result } = renderHook(() => useChatManager());
  await act(async () => { await result.current.handleSendMessage('group', '10001', 'Group reply'); });
  expect(api.sendMessage).toHaveBeenCalledWith('10001', 'Group reply', 'group');
});

it.each(['domain', 'runtime'])('propagates a %s sending failure without adding a message', async kind => {
  if (kind === 'domain') vi.mocked(api.sendMessage).mockResolvedValue(failure);
  else vi.mocked(api.sendMessage).mockRejectedValue(new Error('Network offline'));
  const { result } = renderHook(() => useChatManager());
  await expect(result.current.handleSendMessage('group', '10001', 'Keep this')).rejects.toThrow();
  expect(result.current.chatMessages).toEqual([]);
});

it('preserves the last saved contact rule when persistence is rejected', async () => {
  vi.mocked(api.updateRule).mockResolvedValue(failure);
  const { result } = renderHook(() => useChatManager());
  act(() => result.current.setContacts([contact]));
  await act(async () => { await result.current.handleUpdateMode('10001', 'auto_reply'); });
  expect(result.current.contacts[0].rule.mode).toBe('ignore');
  expect(result.current.actionError).toContain('Storage unavailable');
});

it('applies the returned rule only after confirmed persistence', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.updateRule>>) => void;
  vi.mocked(api.updateRule).mockImplementation(() => new Promise(resolve => { finish = resolve; }) as ReturnType<typeof api.updateRule>);
  const { result } = renderHook(() => useChatManager());
  act(() => result.current.setContacts([contact]));
  let saving!: Promise<void>;
  act(() => { saving = result.current.handleUpdateMode('10001', 'copilot'); });
  expect(result.current.contacts[0].rule.mode).toBe('ignore');
  await act(async () => {
    finish({ success: true, data: { ...contact.rule, mode: 'copilot', enabled: true }, timestamp: 0 });
    await saving;
  });
  expect(result.current.contacts[0].rule.mode).toBe('copilot');
});

it('preserves keywords, cooldown and enabled when the server returns only a mode patch', async () => {
  const saved = { ...contact, rule: { ...contact.rule, keywords: ['help'], cooldownSeconds: 23, enabled: true } };
  vi.mocked(api.updateRule).mockResolvedValue({ success: true, timestamp: 0, data: { mode: 'copilot' } as RoutingRuleDto });
  const { result } = renderHook(() => useChatManager());
  act(() => result.current.setContacts([saved]));
  await act(async () => { await result.current.handleUpdateMode(contact.targetId, 'copilot'); });
  expect(result.current.contacts[0].rule).toEqual({ ...saved.rule, mode: 'copilot' });
});

it('retains a draft when dismissal is refused', async () => {
  vi.mocked(api.dismissDraft).mockResolvedValue(failure);
  const { result } = renderHook(() => useChatManager());
  act(() => result.current.setDrafts([draft]));
  await act(async () => { await result.current.handleDismissDraft(draft.id); });
  expect(result.current.drafts).toEqual([draft]);
  expect(result.current.actionError).toContain('Storage unavailable');
});

it('prevents sending the same draft twice while the first send is pending', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.sendDraft>>) => void;
  vi.mocked(api.sendDraft).mockImplementation(() => new Promise(resolve => { finish = resolve; }) as ReturnType<typeof api.sendDraft>);
  const { result } = renderHook(() => useChatManager());
  act(() => result.current.setDrafts([draft]));
  let sending!: Promise<void>;
  act(() => { sending = result.current.handleSendDraft(draft.id); });
  await act(async () => { await result.current.handleSendDraft(draft.id); });
  expect(api.sendDraft).toHaveBeenCalledTimes(1);
  await act(async () => { finish({ success: true, timestamp: 0, data: { sentMessageId: 'sent-1' } }); await sending; });
  expect(result.current.drafts).toEqual([]);
});

it('ignores history from A after conversation B has loaded', async () => {
  const other = { ...contact, id: 'group-10002', targetId: '10002', name: 'Other group' };
  const message = { id: 'b-1', targetId: other.targetId, timestamp: 1, content: 'B history' } as MessageItemDto;
  let finishA!: (value: Awaited<ReturnType<typeof api.getMessages>>) => void;
  vi.mocked(api.getMessages).mockImplementationOnce(() => new Promise(resolve => { finishA = resolve; }))
    .mockResolvedValueOnce({ success: true, timestamp: 0, data: [message] });
  const { result } = renderHook(() => useChatManager());
  let openingA!: Promise<void>;
  act(() => { openingA = result.current.handleOpenChat(contact); });
  await act(async () => { await result.current.handleOpenChat(other); });
  await act(async () => { finishA({ success: true, timestamp: 0, data: [{ ...message, id: 'a-1', targetId: contact.targetId }] }); await openingA; });
  expect(result.current.selectedChatContact?.targetId).toBe(other.targetId);
  expect(result.current.chatMessages).toEqual([message]);
  expect(api.markRead).toHaveBeenCalledTimes(1);
  expect(api.markRead).toHaveBeenCalledWith(other.targetId, 1);
});

it('does not append a late A send result to B after switching', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.sendMessage>>) => void;
  vi.mocked(api.sendMessage).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const { result } = renderHook(() => useChatManager());
  await act(async () => { await result.current.handleOpenChat(contact); });
  let sending!: Promise<void>;
  act(() => { sending = result.current.handleSendMessage('group', contact.targetId, 'A reply'); });
  await act(async () => { await result.current.handleOpenChat({ ...contact, targetId: '10002' }); });
  await act(async () => { finish({ success: true, timestamp: 0, data: { messageId: 'sent-a' } }); await sending; });
  expect(result.current.chatMessages).toEqual([]);
});

it('invalidates an old send even after closing and reopening the same conversation', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.sendMessage>>) => void;
  vi.mocked(api.sendMessage).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const { result } = renderHook(() => useChatManager());
  await act(async () => { await result.current.handleOpenChat(contact); });
  let sending!: Promise<void>;
  act(() => { sending = result.current.handleSendMessage('group', contact.targetId, 'Previous opening'); });
  act(() => result.current.handleCloseChat());
  await act(async () => { await result.current.handleOpenChat(contact); });
  await act(async () => { finish({ success: true, timestamp: 0, data: { messageId: 'sent-a' } }); await sending; });
  expect(result.current.chatMessages).toEqual([]);
});
