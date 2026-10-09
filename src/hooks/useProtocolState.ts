import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '@/api/client';
import type { ProtocolStatusDto } from '@/api/contracts';

export function useProtocolState(options?: { onLoginSuccess?: () => void }) {
  const [protocolStatus, setProtocolStatus] = useState<ProtocolStatusDto>({ isConnected: false, loginStatus: 'unlogged' });
  const [pendingLogin, setPendingLogin] = useState<{ uin: string; qrcodeBase64: string } | null>(null);
  const [isRefreshingQr, setIsRefreshingQr] = useState(false);
  const [qrError, setQrError] = useState<string | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [isQuickLoggingIn, setIsQuickLoggingIn] = useState<string | null>(null);
  const [chainHasFailure, setChainHasFailure] = useState(false);
  const generation = useRef(0);
  const busy = useRef(false);
  const polling = useRef(false);
  const mounted = useRef(true);
  const previous = useRef('unlogged');
  const onSuccess = useRef(options?.onLoginSuccess);
  onSuccess.current = options?.onLoginSuccess;

  const refreshChainStatus = useCallback(async () => {
    const epoch = generation.current;
    try {
      const res = await api.getChainStatus();
      if (mounted.current && epoch === generation.current && res.success && res.data) setChainHasFailure(res.data.hasFailure);
    } catch { /* A protocol refresh reports actionable errors. */ }
  }, []);

  const refreshProtocolStatus = useCallback(async () => {
    if (busy.current || polling.current) return;
    polling.current = true;
    const epoch = generation.current;
    const watchdog = setTimeout(() => {
      if (mounted.current && epoch === generation.current) setStatusError('状态查询超过 15 秒，二维码服务未响应。请查看链路诊断。');
    }, 15000);
    try {
      const res = await api.getProtocolStatus();
      if (!mounted.current || epoch !== generation.current) return;
      if (!res.success || !res.data) throw new Error(res.error?.message || '协议状态查询失败');
      setStatusError(null);
      const next = res.data;
      setProtocolStatus(next);
      if (next.loginStatus === 'logged_in' && previous.current !== 'logged_in') onSuccess.current?.();
      previous.current = next.loginStatus;
    } catch (e) {
      if (mounted.current && epoch === generation.current) setStatusError(e instanceof Error ? e.message : String(e));
    } finally { clearTimeout(watchdog); polling.current = false; }
  }, []);

  const mutate = useCallback(async (operation: () => Promise<void>) => {
    if (busy.current) return;
    busy.current = true; generation.current++; setQrError(null);
    try { await operation(); }
    catch (e) { if (mounted.current) setQrError(e instanceof Error ? e.message : String(e)); }
    finally {
      busy.current = false;
      if (mounted.current) { setIsRefreshingQr(false); setIsQuickLoggingIn(null); }
      await refreshProtocolStatus();
    }
  }, [refreshProtocolStatus]);

  const handleRefreshQr = useCallback(() => mutate(async () => {
    setIsRefreshingQr(true);
    const res = await api.refreshQrCode();
    if (!res.success || !res.data) throw new Error(res.error?.message || '二维码刷新失败');
    if (mounted.current) setProtocolStatus(prev => ({ ...prev, qrcodeBase64: res.data!.qrcodeBase64, loginStatus: 'waiting_scan' }));
  }), [mutate]);

  const handleRestoreProtocol = useCallback(() => mutate(async () => {
    setIsRefreshingQr(true);
    const res = await api.restartNapCat();
    if (!res.success || !res.data?.ok) throw new Error(res.error?.message || res.data?.detail || '协议恢复失败');
  }), [mutate]);

  const handleQuickLogin = useCallback((uin: string) => mutate(async () => {
    setIsQuickLoggingIn(uin);
    const res = await api.quickLogin(uin);
    if (!res.success) {
      if (res.error?.code === 1005) {
        const qr = await api.accountQrCode(uin);
        if (qr.success && qr.data?.qrcodeBase64 && mounted.current) {
          setPendingLogin({ uin, qrcodeBase64: qr.data.qrcodeBase64 });
        }
      }
      throw new Error(res.error?.message || '目标账号登录未确认');
    }
    if (mounted.current) setPendingLogin(null);
  }), [mutate]);

  const handleLogout = useCallback(() => mutate(async () => {
    const res = await api.logout();
    if (!res.success) throw new Error(res.error?.message || '退出登录失败');
    if (mounted.current) setProtocolStatus({ isConnected: false, loginStatus: 'unlogged' });
    previous.current = 'unlogged';
  }), [mutate]);

  useEffect(() => {
    mounted.current = true;
    let timer: ReturnType<typeof setTimeout>;
    let cancelled = false;
    let tick = 0;
    const poll = async () => {
      await refreshProtocolStatus();
      if (tick++ % 4 === 0) void refreshChainStatus();
      if (!cancelled) timer = setTimeout(poll, 2000);
    };
    void poll();
    return () => { cancelled = true; mounted.current = false; generation.current++; clearTimeout(timer); };
  }, [refreshProtocolStatus, refreshChainStatus]);

  return { pendingLogin, setPendingLogin, protocolStatus, setProtocolStatus, isRefreshingQr, qrError: qrError || statusError, isQuickLoggingIn, chainHasFailure,
    handleRefreshQr, handleRestoreProtocol, handleQuickLogin, handleLogout, refreshProtocolStatus, refreshChainStatus };
}
