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
  const versionEpoch = useRef(0);
  const pending = useRef(false);
  const unsubscribe = useRef<(() => void) | null>(null);
  useEffect(() => {
    mounted.current = true;
    const epoch = ++versionEpoch.current;
    api.getNapCatVersion().then(result => { if (mounted.current && epoch === versionEpoch.current && result.success && result.data) setVersion(result.data); }).catch(() => {});
    return () => { mounted.current = false; versionEpoch.current++; unsubscribe.current?.(); };
  }, []);
  const check = async () => {
    if (pending.current) return;
    versionEpoch.current++;
    pending.current = true; setBusy('check'); setError(null); setNotice(null); setInfo(null); setProgress(null);
    try {
      const result = await api.checkNapCatUpdate();
      if (!mounted.current) return;
      if (!result.success || !result.data) throw new Error(result.error?.message || '检查协议更新失败');
      setInfo(result.data); setVersion(result.data.currentVersion);
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { window.dispatchEvent(new Event('eazyqq-updates-changed')); pending.current = false; if (mounted.current) setBusy(null); }
  };
  const install = async () => {
    if (pending.current || !info?.hasUpdate || !info.downloadUrl) return;
    versionEpoch.current++;
    pending.current = true; setBusy('install'); setError(null); setNotice(null); setProgress(null);
    let operationId: string | undefined;
    try {
      const unlisten = await api.onNapCatUpdateProgress(value => {
        if (!mounted.current) return;
        if (value.operationId) {
          if (!operationId && value.phase !== 'checking') return;
          if (operationId && operationId !== value.operationId) return;
          operationId = value.operationId;
        }
        setProgress(value);
      });
      unsubscribe.current = unlisten;
      if (!mounted.current) { unlisten(); return; }
      const result = await api.upgradeNapCat(info.downloadUrl);
      if (!mounted.current) return;
      if (!result.success) throw new Error(result.error?.message || '协议升级失败');
      setNotice('核心文件已部署，版本尚未确认；运行状态需启动后核验。');
      setVersion('unknown');
      setInfo(previous => previous ? { ...previous, currentVersion: 'unknown', hasUpdate: true, status: 'version_unknown' } : null);
      const current = await api.getNapCatVersion();
      if (!mounted.current) return;
      if (!current.success || !current.data) throw new Error('核心文件已部署，但版本读取失败；请重新检查更新');
      setVersion(current.data);
      setInfo(previous => previous ? { ...previous, currentVersion:current.data!, hasUpdate:current.data !== previous.latestVersion,
        status:current.data === previous.latestVersion ? 'up_to_date' : 'version_unknown' } : null);
      if (current.data !== 'unknown') setNotice(`核心文件已部署，本机读取版本 v${current.data}；运行状态需启动后核验。`);
    } catch (cause) {
      if (mounted.current) { setError(cause instanceof Error ? cause.message : String(cause)); setProgress(null); }
    } finally {
      window.dispatchEvent(new Event('eazyqq-updates-changed'));
      unsubscribe.current?.(); unsubscribe.current = null; pending.current = false;
      if (mounted.current) setBusy(null);
    }
  };
  return { version, info, busy, error, notice, progress, check, install };
}
