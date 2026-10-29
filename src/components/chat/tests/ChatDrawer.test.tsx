import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ChatDrawer } from '../ChatDrawer';
import type { ContactItemDto } from '@/api/contracts';

vi.mock('@/hooks/usePresence', () => ({ usePresence: (open: boolean) => open }));
const contact = { id: 'group-10001', targetId: '10001', targetType: 'group', name: 'Group', rule: { mode: 'ignore' } } as ContactItemDto;
const props = { isOpen: true, onClose: vi.fn(), contact, messages: [] };
beforeEach(() => { Element.prototype.scrollIntoView = vi.fn(); });
afterEach(cleanup);

it('keeps text and shows a retryable error when manual sending is rejected', async () => {
  const send = vi.fn().mockRejectedValueOnce(new Error('Identity not confirmed')).mockResolvedValue(undefined);
  render(<ChatDrawer {...props} onSendMessage={send} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'Keep this reply' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '发送消息' })); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('Keep this reply');
  expect(screen.getByRole('alert').textContent).toContain('Identity not confirmed');
  expect(send).toHaveBeenCalledWith('group', '10001', 'Keep this reply');
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '发送消息' })); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('');
  expect(screen.queryByRole('alert')).toBeNull();
});

it('does not erase text entered while a previous message is being sent', async () => {
  let finish!: () => void;
  const send = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
  render(<ChatDrawer {...props} onSendMessage={send} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'First message' } });
  fireEvent.click(screen.getByRole('button', { name: '发送消息' }));
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'Next message' } });
  await act(async () => { finish(); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('Next message');
  expect(send).toHaveBeenCalledTimes(1);
});

it('does not put a late AI response for A into B and preserves both drafts', async () => {
  let finish!: (value: string) => void;
  const generate = vi.fn(() => new Promise<string>(resolve => { finish = resolve; }));
  const send = vi.fn().mockResolvedValue(undefined);
  const view = render(<ChatDrawer {...props} onSendMessage={send} onTriggerAiReply={generate} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'A draft' } });
  fireEvent.click(screen.getByRole('button', { name: '让 AI 替我构思一条回复' }));
  view.rerender(<ChatDrawer {...props} contact={{ ...contact, targetId: '10002', name: 'Group B' }} onSendMessage={send} onTriggerAiReply={generate} />);
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('');
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'B draft' } });
  await act(async () => { finish('Late answer for A'); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('B draft');
  view.rerender(<ChatDrawer {...props} onSendMessage={send} onTriggerAiReply={generate} />);
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('A draft');
});

it('does not clear Bs text when As pending send completes', async () => {
  let finish!: () => void;
  const send = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
  const view = render(<ChatDrawer {...props} onSendMessage={send} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'Same words' } });
  fireEvent.click(screen.getByRole('button', { name: '发送消息' }));
  view.rerender(<ChatDrawer {...props} contact={{ ...contact, targetId: '10002' }} onSendMessage={send} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'Same words' } });
  await act(async () => { finish(); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('Same words');
});

it('ignores an AI response from a previous opening of the same conversation', async () => {
  let finish!: (value: string) => void;
  const generate = vi.fn(() => new Promise<string>(resolve => { finish = resolve; }));
  const send = vi.fn().mockResolvedValue(undefined);
  const view = render(<ChatDrawer {...props} onSendMessage={send} onTriggerAiReply={generate} />);
  fireEvent.change(screen.getByLabelText('消息内容'), { target: { value: 'Keep A draft' } });
  fireEvent.click(screen.getByRole('button', { name: '让 AI 替我构思一条回复' }));
  view.rerender(<ChatDrawer {...props} isOpen={false} onSendMessage={send} onTriggerAiReply={generate} />);
  view.rerender(<ChatDrawer {...props} onSendMessage={send} onTriggerAiReply={generate} />);
  await act(async () => { finish('Old opening answer'); });
  expect((screen.getByLabelText('消息内容') as HTMLTextAreaElement).value).toBe('Keep A draft');
});
