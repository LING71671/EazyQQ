import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { LoginView } from '../LoginView';

vi.mock('@/components/accounts/AccountManager', () => ({ AccountManager: () => null }));
vi.mock('@/components/accounts/AccountLoginQr', () => ({ AccountLoginQr: () => null }));
beforeEach(() => vi.useFakeTimers());
afterEach(() => { cleanup(); vi.useRealTimers(); });

it('exposes an offline payload error and uses explicit recovery instead of QR refresh', () => {
  const refresh = vi.fn(); const restore = vi.fn();
  render(<LoginView status={{ isConnected: false, loginStatus: 'unlogged', qrcodeError: '协议文件缺失：napcat.mjs' }} isLoading={false} onRefreshQr={refresh} onRestoreProtocol={restore} />);
  expect(screen.getByRole('alert').textContent).toContain('napcat.mjs');
  expect(screen.queryByText('服务加载中')).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: '恢复协议' }));
  expect(restore).toHaveBeenCalledTimes(1);
  expect(refresh).not.toHaveBeenCalled();
});

it('ends the loading presentation after twenty seconds without mutating a session', async () => {
  const refresh = vi.fn(); const restore = vi.fn();
  render(<LoginView status={{ isConnected: false, loginStatus: 'unlogged' }} isLoading={false} onRefreshQr={refresh} onRestoreProtocol={restore} />);
  await act(async () => { await vi.advanceTimersByTimeAsync(20000); });
  expect(screen.getByRole('alert').textContent).toContain('20 秒');
  expect(screen.queryByText('服务加载中')).toBeNull();
  expect(restore).not.toHaveBeenCalled();
  expect(refresh).not.toHaveBeenCalled();
});

it('keeps an available QR and refresh action after a successful protocol response', async () => {
  const refresh = vi.fn();
  render(<LoginView status={{ isConnected: true, loginStatus: 'waiting_scan', qrcodeBase64: 'data:image/png;base64,Zml4dHVyZQ==' }} isLoading={false} onRefreshQr={refresh} />);
  await act(async () => { await Promise.resolve(); await vi.advanceTimersByTimeAsync(21000); });
  expect(screen.getByAltText('Login QR Code')).toBeTruthy();
  expect(screen.queryByRole('alert')).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: '刷新二维码' }));
  expect(refresh).toHaveBeenCalledTimes(1);
});

it('keeps quick login explicit and exposes batch management beside the QR workspace', () => {
  const login = vi.fn();
  render(<LoginView status={{ isConnected: true, loginStatus: 'waiting_scan', quickLoginAccounts: [{ uin: '10001', nickname: 'Remembered account' }] }} isLoading={false} onRefreshQr={vi.fn()} onQuickLogin={login} />);
  const button = screen.getByRole('button', { name: '一键登录 10001' });
  expect(button.querySelector('svg')).toBeNull();
  expect(login).not.toHaveBeenCalled();
  expect(screen.getByText('账号管理与批量操作').closest('details')?.open).toBe(false);
  fireEvent.click(button);
  expect(login).toHaveBeenCalledWith('10001');
});
