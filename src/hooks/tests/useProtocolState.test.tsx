import { act, cleanup, renderHook } from '@testing-library/react';
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProtocolState } from '../useProtocolState';
import { api } from '@/api/client';
import type { ApiResponse, ProtocolStatusDto } from '@/api/contracts';

vi.mock('@/api/client', () => ({ api: {
  getAccountStatus: vi.fn(), accountQrCode: vi.fn(), getProtocolStatus: vi.fn(), getChainStatus: vi.fn(), quickLogin: vi.fn(), refreshQrCode: vi.fn(), restartNapCat: vi.fn(), logout: vi.fn(),
} }));
const loggedIn: ApiResponse<ProtocolStatusDto> = { success: true, timestamp: 0, data: { isConnected: true, loginStatus: 'logged_in', qqNumber: '10001' } };
const pending: ApiResponse<ProtocolStatusDto> = { success: true, timestamp: 0, data: { isConnected: true, loginStatus: 'waiting_scan', qrcodeBase64: 'old-qr' } };
const flush = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });

beforeEach(() => {
  vi.useFakeTimers(); vi.clearAllMocks();
  vi.mocked(api.getProtocolStatus).mockResolvedValue(loggedIn);
  vi.mocked(api.getChainStatus).mockResolvedValue({ success: true, timestamp: 0, data: { hasFailure: false, links: [], firstBreak: null, uptimeSecs: 0 } });
  vi.mocked(api.quickLogin).mockResolvedValue({ success: true, timestamp: 0 });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('protocol account transitions', () => {
  it('opens the target QR after expired credentials while retaining the current account', async () => {
    vi.mocked(api.quickLogin).mockResolvedValue({ success: false, timestamp: 0, error: { code: 1005, message: 'Expired login' } });
    vi.mocked(api.accountQrCode).mockResolvedValue({ success: true, timestamp: 0, data: { uin: '10002', qrcodeBase64: 'https://example.test/target-qr' } });
    const { result } = renderHook(() => useProtocolState());
    await flush();
    await act(async () => { await result.current.handleQuickLogin('10002'); });
    expect(result.current.pendingLogin?.uin).toBe('10002');
    expect(result.current.protocolStatus.qqNumber).toBe('10001');
    expect(api.accountQrCode).toHaveBeenCalledWith('10002');
  });
  it('does not overlap slow polling calls', async () => {
    vi.mocked(api.getProtocolStatus).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useProtocolState());
    await act(async () => { await vi.advanceTimersByTimeAsync(20000); });
    expect(api.getProtocolStatus).toHaveBeenCalledTimes(1);
    expect(result.current.qrError).toContain('15 秒');
    expect(api.quickLogin).not.toHaveBeenCalled();
    expect(api.restartNapCat).not.toHaveBeenCalled();
  });
  it('shows domain failures and clears them after a successful read-only poll', async () => {
    vi.mocked(api.getProtocolStatus).mockResolvedValueOnce({ success: false, timestamp: 0, error: { code: 1001, message: 'Protocol unavailable' } });
    const { result } = renderHook(() => useProtocolState());
    await flush();
    expect(result.current.qrError).toBe('Protocol unavailable');
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(result.current.qrError).toBeNull();
  });
  it('restores the protocol only after an explicit action and reports refused restarts', async () => {
    vi.mocked(api.restartNapCat).mockResolvedValue({ success: true, timestamp: 0, data: { attempted: false, ok: false, detail: 'Unowned session preserved' } });
    const { result } = renderHook(() => useProtocolState());
    await flush();
    expect(api.restartNapCat).not.toHaveBeenCalled();
    await act(async () => { await result.current.handleRestoreProtocol(); });
    expect(api.restartNapCat).toHaveBeenCalledTimes(1);
    expect(result.current.qrError).toBe('Unowned session preserved');
    expect(result.current.isRefreshingQr).toBe(false);
  });
  it('discards a stale QR response after an account operation', async () => {
    let resolve!: (value: ApiResponse<ProtocolStatusDto>) => void;
    vi.mocked(api.getProtocolStatus).mockReturnValueOnce(new Promise(done => { resolve = done; })).mockResolvedValue(loggedIn);
    const { result } = renderHook(() => useProtocolState());
    await act(async () => { await result.current.handleQuickLogin('10001'); });
    await act(async () => { resolve(pending); });
    expect(result.current.protocolStatus.qrcodeBase64).toBeUndefined();
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(result.current.protocolStatus.loginStatus).toBe('logged_in');
  });
  it('serializes double-click login requests', async () => {
    vi.mocked(api.quickLogin).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useProtocolState());
    await flush();
    act(() => { void result.current.handleQuickLogin('10002'); void result.current.handleQuickLogin('10003'); });
    expect(api.quickLogin).toHaveBeenCalledTimes(1);
    expect(result.current.isQuickLoggingIn).toBe('10002');
  });
  it('retains the authenticated account when switching fails', async () => {
    vi.mocked(api.quickLogin).mockResolvedValue({ success: false, timestamp: 0, error: { code: 1002, message: 'Login timeout' } });
    const { result } = renderHook(() => useProtocolState());
    await flush();
    await act(async () => { await result.current.handleQuickLogin('10002'); });
    expect(result.current.protocolStatus.qqNumber).toBe('10001');
    expect(result.current.qrError).toBe('Login timeout');
  });
  it('reports failed logout without claiming the session was closed', async () => {
    vi.mocked(api.logout).mockResolvedValue({ success: false, timestamp: 0, error: { code: 1003, message: 'Unowned session' } });
    const { result } = renderHook(() => useProtocolState());
    await flush();
    await act(async () => { await result.current.handleLogout(); });
    expect(result.current.protocolStatus.loginStatus).toBe('logged_in');
    expect(result.current.qrError).toBe('Unowned session');
  });
  it('calls login completion once under StrictMode and repeated polls', async () => {
    const onLoginSuccess = vi.fn();
    renderHook(() => useProtocolState({ onLoginSuccess }), { wrapper: StrictMode });
    await flush();
    await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
    expect(onLoginSuccess).toHaveBeenCalledTimes(1);
  });
});
