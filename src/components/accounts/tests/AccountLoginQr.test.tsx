import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { AccountLoginQr } from '../AccountLoginQr';
import { api } from '@/api/client';
import type { AccountReport, ApiResponse } from '@/api/contracts';

vi.mock('@/api/client', () => ({ api: { getAccountStatus: vi.fn(), accountQrCode: vi.fn() } }));
vi.mock('@/features/accounts/qrImage', () => ({ qrImage: async (value: string) => value }));
const pending = { uin: '10002', qrcodeBase64: 'data:image/png;base64,current' };
const flush = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });
const report = (uin: string): ApiResponse<AccountReport> => ({ success: true, timestamp: 0, data: {
  instance: { uin, httpPort: 3002, wsPort: 3003, webuiPort: 6100, autoStart: false, processManaged: true },
  login: { loggedIn: true, uin, source: 'onebot' }, selected: false,
} });
beforeEach(() => { vi.useFakeTimers(); vi.clearAllMocks(); vi.mocked(api.getAccountStatus).mockResolvedValue(report('10003')); });
afterEach(() => { cleanup(); vi.useRealTimers(); });

it('confirms only the requested identity once', async () => {
  const confirmed = vi.fn();
  render(<AccountLoginQr pending={pending} onConfirmed={confirmed} onCancel={() => {}} />);
  await flush();
  expect(confirmed).not.toHaveBeenCalled();
  vi.mocked(api.getAccountStatus).mockResolvedValue(report('10002'));
  await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
  expect(confirmed).toHaveBeenCalledExactlyOnceWith('10002');
});

it('discards a refresh completed after changing the target account', async () => {
  let resolve!: (value: Awaited<ReturnType<typeof api.accountQrCode>>) => void;
  vi.mocked(api.accountQrCode).mockReturnValue(new Promise(done => { resolve = done; }));
  const props = { onConfirmed: vi.fn(), onCancel: vi.fn() };
  const { rerender } = render(<AccountLoginQr pending={pending} {...props} />);
  await flush();
  fireEvent.click(screen.getByRole('button', { name: '刷新二维码' }));
  rerender(<AccountLoginQr pending={{ uin: '10004', qrcodeBase64: 'data:image/png;base64,next' }} {...props} />);
  await flush();
  await act(async () => { resolve({ success: true, timestamp: 0, data: { uin: '10002', qrcodeBase64: 'data:image/png;base64,stale' } }); });
  expect(screen.getByRole('img').getAttribute('src')).toBe('data:image/png;base64,next');
});

it('does not confirm a response arriving after cancellation', async () => {
  let resolve!: (value: ApiResponse<AccountReport>) => void;
  vi.mocked(api.getAccountStatus).mockReturnValue(new Promise(done => { resolve = done; }));
  const confirmed = vi.fn();
  const { unmount } = render(<AccountLoginQr pending={pending} onConfirmed={confirmed} onCancel={() => {}} />);
  unmount();
  await act(async () => { resolve(report('10002')); });
  expect(confirmed).not.toHaveBeenCalled();
});
