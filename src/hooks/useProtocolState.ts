import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '@/api/client';
import type { ProtocolStatusDto } from '@/api/contracts';

interface UseProtocolStateOptions {
  onLoginSuccess?: () => void;
}

export function useProtocolState(options?: UseProtocolStateOptions) {
  const [protocolStatus, setProtocolStatus] = useState<ProtocolStatusDto>({
    isConnected: false,
    loginStatus: 'waiting_scan',
    qqNumber: '',
    nickname: '',
  });

  const [isRefreshingQr, setIsRefreshingQr] = useState(false);
  const [qrError, setQrError] = useState<string | null>(null);
  const [isQuickLoggingIn, setIsQuickLoggingIn] = useState<string | null>(null);
  const [chainHasFailure, setChainHasFailure] = useState(false);

  const prevLoginStatusRef = useRef(protocolStatus.loginStatus);
  const onLoginSuccessRef = useRef(options?.onLoginSuccess);

  useEffect(() => {
    onLoginSuccessRef.current = options?.onLoginSuccess;
  }, [options?.onLoginSuccess]);

  const refreshChainStatus = useCallback(async () => {
    try {
      const res = await api.getChainStatus();
      if (res.success && res.data) {
        setChainHasFailure(res.data.hasFailure);
      }
    } catch {
      // Non-blocking chain probe
    }
  }, []);

  const refreshProtocolStatus = useCallback(async () => {
    try {
      const res = await api.getProtocolStatus();
      if (res.success && res.data) {
        const nextStatus = res.data;
        setProtocolStatus((prev) => {
          const effectiveQr =
            nextStatus.loginStatus === 'logged_in'
              ? undefined
              : nextStatus.qrcodeBase64 || prev.qrcodeBase64;

          const mergedStatus: ProtocolStatusDto = {
            ...nextStatus,
            qrcodeBase64: effectiveQr,
          };

          if (
            nextStatus.loginStatus === 'logged_in' &&
            prevLoginStatusRef.current !== 'logged_in'
          ) {
            onLoginSuccessRef.current?.();
          }
          prevLoginStatusRef.current = nextStatus.loginStatus;

          if (
            prev.loginStatus !== mergedStatus.loginStatus ||
            prev.qqNumber !== mergedStatus.qqNumber ||
            prev.qrcodeBase64 !== mergedStatus.qrcodeBase64 ||
            prev.nickname !== mergedStatus.nickname
          ) {
            return mergedStatus;
          }
          return prev;
        });
      }
    } catch {
      // Protocol offline or starting up
    }
  }, []);

  const handleRefreshQr = useCallback(async () => {
    setIsRefreshingQr(true);
    setQrError(null);
    try {
      const res = await api.refreshQrCode();
      if (res.success && res.data) {
        const qr = res.data;
        setProtocolStatus((prev) => ({
          ...prev,
          qrcodeBase64: qr.qrcodeBase64,
          loginStatus: 'waiting_scan',
        }));
      } else {
        setQrError(res.error?.message || '获取全新二维码失败，请稍后重试');
      }
    } catch (e) {
      setQrError(e instanceof Error ? e.message : String(e));
      console.error('Failed to refresh QR code', e);
    } finally {
      setIsRefreshingQr(false);
    }
  }, []);

  const handleQuickLogin = useCallback(async (uin: string) => {
    setIsQuickLoggingIn(uin);
    setQrError(null);
    try {
      const res = await api.quickLogin(uin);
      if (res.success) {
        const sRes = await api.getProtocolStatus();
        if (sRes.success && sRes.data) {
          setProtocolStatus(sRes.data);
          if (sRes.data.loginStatus === 'logged_in') {
            onLoginSuccessRef.current?.();
          }
        }
      } else {
        setQrError(res.error?.message || '快速登录未成功，请稍候重试');
      }
    } catch (e) {
      setQrError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsQuickLoggingIn(null);
    }
  }, []);

  // Periodic polling for scan/login and chain status
  useEffect(() => {
    // Initial fetch
    refreshChainStatus();
    refreshProtocolStatus();

    let pollTick = 0;
    const interval = setInterval(() => {
      pollTick++;
      if (pollTick % 4 === 0) {
        refreshChainStatus();
      }
      refreshProtocolStatus();
    }, 2000);

    return () => clearInterval(interval);
  }, [refreshChainStatus, refreshProtocolStatus]);

  return {
    protocolStatus,
    setProtocolStatus,
    isRefreshingQr,
    qrError,
    isQuickLoggingIn,
    chainHasFailure,
    handleRefreshQr,
    handleQuickLogin,
    refreshProtocolStatus,
    refreshChainStatus,
  };
}
