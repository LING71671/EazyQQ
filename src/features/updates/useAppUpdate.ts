import { useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { AppUpdateInfo, AppUpdateProgress } from '@/api/contracts';

export function useAppUpdate() {
  const [info, setInfo] = useState<AppUpdateInfo | null>(null);
  const [busy, setBusy] = useState<'check' | 'install' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const mounted = useRef(true);
  const pending = useRef(false);
  const cleanup = useRef<(() => void) | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; cleanup.current?.(); }; }, []);

  const check = async () => {
    if (pending.current) return;
    pending.current = true; setBusy('check'); setError(null); setNotice(null); setInfo(null); setProgress(null);
    try {
      const result = await api.checkAppUpdate();
      if (!mounted.current) return;
      if (!result.success || !result.data) throw new Error(result.error?.message || '检查更新失败');
      setInfo(result.data);
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      pending.current = false; if (mounted.current) setBusy(null);
    }
  };

  const install = async () => {
    if (pending.current || !info?.hasUpdate || !info.downloadUrl || !info.checksumSha256) return;
    pending.current = true; setBusy('install'); setError(null); setNotice(null); setProgress(null);
    try {
      const unlisten = await api.onAppUpdateProgress(value => { if (mounted.current) setProgress(value); });
      cleanup.current = unlisten;
      if (!mounted.current) { unlisten(); return; }
      const result = await api.upgradeApp(info.downloadUrl);
      if (!mounted.current) return;
      if (!result.success) throw new Error(result.error?.message || '安装启动失败');
      setNotice(result.data || '安装包已校验，程序即将退出以完成更新。');
    } catch (cause) {
      if (mounted.current) { setError(cause instanceof Error ? cause.message : String(cause)); setProgress(null); }
    } finally {
      cleanup.current?.(); cleanup.current = null;
      pending.current = false; if (mounted.current) setBusy(null);
    }
  };
  return { info, busy, error, notice, progress, check, install };
}
