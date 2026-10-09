import { useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { AppUpdateProgress, NapCatUpdateInfo } from '@/api/contracts';

export function useCoreUpdate() {
  const [version, setVersion] = useState('unknown');
  const [info, setInfo] = useState<NapCatUpdateInfo | null>(null);
  const [busy, setBusy] = useState<'check' | 'install' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const mounted = useRef(true);
  const pending = useRef(false);
  const unsubscribe = useRef<(() => void) | null>(null);
  useEffect(() => {
    mounted.current = true;
    api.getNapCatVersion().then(result => { if (mounted.current && result.success && result.data) setVersion(result.data); }).catch(() => {});
    return () => { mounted.current = false; unsubscribe.current?.(); };
  }, []);
  const check = async () => {
    if (pending.current) return;
    pending.current = true; setBusy('check'); setError(null); setNotice(null); setInfo(null); setProgress(null);
    try {
      const result = await api.checkNapCatUpdate();
      if (!mounted.current) return;
      if (!result.success || !result.data) throw new Error(result.error?.message || '检查协议更新失败');
      setInfo(result.data); setVersion(result.data.currentVersion);
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { pending.current = false; if (mounted.current) setBusy(null); }
  };
  const install = async () => {
    if (pending.current || !info?.hasUpdate || !info.downloadUrl) return;
    pending.current = true; setBusy('install'); setError(null); setNotice(null); setProgress(null);
    try {
      const unlisten = await api.onNapCatUpdateProgress(value => { if (mounted.current) setProgress(value); });
      unsubscribe.current = unlisten;
      if (!mounted.current) { unlisten(); return; }
      const result = await api.upgradeNapCat(info.downloadUrl);
      if (!mounted.current) return;
      if (!result.success) throw new Error(result.error?.message || '协议升级失败');
      setNotice(result.data || '协议更新完成，请启动所需账号。');
      const current = await api.getNapCatVersion();
      if (!mounted.current) return;
      if (!current.success || !current.data) throw new Error('更新已完成，但版本读取失败；请重新检查更新');
      setVersion(current.data);
      setInfo(previous => previous ? { ...previous, currentVersion:current.data!, hasUpdate:current.data !== previous.latestVersion,
        status:current.data === previous.latestVersion ? 'up_to_date' : 'version_unknown' } : null);
    } catch (cause) {
      if (mounted.current) { setError(cause instanceof Error ? cause.message : String(cause)); setProgress(null); }
    } finally {
      unsubscribe.current?.(); unsubscribe.current = null; pending.current = false;
      if (mounted.current) setBusy(null);
    }
  };
  return { version, info, busy, error, notice, progress, check, install };
}
