import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { PersonaCard } from '../settings/PersonaCard';
import type { ContactItemDto } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ manage: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { managePersonas: mocks.manage } }));
const response = (text = '', inheritedText = '') => ({ success: true, data: { text, inheritedText, revision: 2, overrides: [] }, timestamp: 1 });
const contacts = [{ id: 'group:20001', targetType: 'group', targetId: '20001', name: '项目群' }] as ContactItemDto[];
beforeEach(() => { mocks.manage.mockReset(); mocks.manage.mockResolvedValue(response()); });
afterEach(cleanup);

it('saves an account override and restores inheritance only after persistence', async () => {
  render(<PersonaCard accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.change(screen.getByLabelText('表达偏好正文'), { target: { value: '短句，先回应重点' } });
  mocks.manage.mockResolvedValueOnce(response('短句，先回应重点'));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存表达偏好' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'set', text: '短句，先回应重点' });
  mocks.manage.mockResolvedValueOnce(response('', '默认表达偏好'));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '恢复继承' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'clear' });
  expect((screen.getByLabelText('表达偏好正文') as HTMLTextAreaElement).value).toBe('');
  expect(screen.getByRole('status').textContent).toBe('已恢复继承');
});

it('binds a member override to its selected group and stable QQ number', async () => {
  render(<PersonaCard accountUin="10001" contacts={contacts} />);
  await act(async () => { await Promise.resolve(); });
  fireEvent.change(screen.getByLabelText('表达偏好范围'), { target: { value: 'member' } });
  fireEvent.change(screen.getByLabelText('表达偏好会话'), { target: { value: '20001' } });
  await act(async () => { fireEvent.change(screen.getByLabelText('表达偏好成员QQ'), { target: { value: '30001' } }); });
  expect(mocks.manage).toHaveBeenLastCalledWith({ operation: 'get', targetType: 'group', targetId: '20001', memberUin: '30001' });
});

it('ignores a late scope read and keeps failed edits visible for correction', async () => {
  let finish!: (value: unknown) => void;
  mocks.manage.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<PersonaCard accountUin="10001" contacts={contacts} />);
  fireEvent.change(screen.getByLabelText('表达偏好范围'), { target: { value: 'group' } });
  await act(async () => { fireEvent.change(screen.getByLabelText('表达偏好会话'), { target: { value: '20001' } }); });
  await act(async () => { finish(response('旧账号偏好')); });
  expect((screen.getByLabelText('表达偏好正文') as HTMLTextAreaElement).value).toBe('');
  fireEvent.change(screen.getByLabelText('表达偏好正文'), { target: { value: '新群偏好' } });
  mocks.manage.mockResolvedValueOnce({ success: false, error: { message: '保存失败' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存表达偏好' })); });
  expect(screen.getByRole('alert').textContent).toBe('保存失败');
  expect((screen.getByLabelText('表达偏好正文') as HTMLTextAreaElement).value).toBe('新群偏好');
});
