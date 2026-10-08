import { useEffect, useRef, useState } from 'react';
import { qrImage } from '@/features/accounts/qrImage';
import { api } from '@/api/client';

export function AccountLoginQr({ pending, onConfirmed, onCancel }: {
  pending: { uin: string; qrcodeBase64: string };
  onConfirmed: (uin: string) => void;
  onCancel: () => void;
}) {
  const [image, setImage] = useState('');
  const [error, setError] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const confirmed = useRef(false);
  const generation = useRef(0);
  const refreshInFlight = useRef(false);
  const onConfirmedRef = useRef(onConfirmed);
  onConfirmedRef.current = onConfirmed;

  useEffect(() => {
    let active = true;
    generation.current += 1;
    refreshInFlight.current = false;
    setRefreshing(false); setError(''); setImage('');
    const value = pending.qrcodeBase64;
    const converted = qrImage(value);
    converted.then(result => { if (active) setImage(result); }).catch(() => { if (active) setError('二维码生成失败，请刷新'); });
    return () => { active = false; generation.current += 1; };
  }, [pending.uin, pending.qrcodeBase64]);

  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    confirmed.current = false;
    const poll = async () => {
      try {
        const result = await api.getAccountStatus(pending.uin);
        if (!stopped && result.success && result.data?.login.loggedIn && result.data.login.uin === pending.uin && !confirmed.current) {
          confirmed.current = true;
          onConfirmedRef.current(pending.uin);
        }
      } catch { /* The next poll retries transient control-plane failures. */ }
      if (!stopped && !confirmed.current) timer = setTimeout(poll, 2000);
    };
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, [pending.uin]);

  const refresh = async () => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    const current = generation.current;
    const isCurrent = () => current === generation.current;
    setRefreshing(true); setError('');
    try {
      const result = await api.accountQrCode(pending.uin, true);
      if (!isCurrent()) return;
      if (result.success && result.data?.loggedIn) {
        // Read-only polling confirms the exact identity before committing selection.
        return;
      }
      if (!result.success || !result.data?.qrcodeBase64) throw new Error(result.error?.message || '二维码未就绪');
      const value = result.data.qrcodeBase64;
      const converted = await qrImage(value);
      if (isCurrent()) setImage(converted);
    } catch (e) { if (isCurrent()) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (isCurrent()) { refreshInFlight.current = false; setRefreshing(false); } }
  };

  return <section aria-label="目标账号扫码" className="mt-6 w-full max-w-lg rounded-2xl border border-sky-200 bg-white p-5 text-center dark:border-slate-700 dark:bg-slate-900">
    <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">账号 {pending.uin} 扫码登录</h3>
    <p className="mt-1 text-sm text-slate-600 dark:text-slate-300">当前账号保持连接，扫码确认后自动切换</p>
    {image && <img src={image} alt={`QQ ${pending.uin} 登录二维码`} width={256} height={256} className="mx-auto mt-3" />}
    <p role="status" className="mt-2 break-words text-sm text-rose-700">{error}</p>
    <div className="mt-3 flex justify-center gap-3">
      <button type="button" disabled={refreshing} onClick={() => void refresh()} className="rounded-lg bg-sky-600 px-3 py-2 text-sm text-white hover:bg-sky-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:opacity-50">{refreshing ? '刷新中…' : '刷新二维码'}</button>
      <button type="button" onClick={onCancel} className="rounded-lg border border-slate-300 px-3 py-2 text-sm text-slate-700 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-sky-600 dark:text-slate-200">取消切换</button>
    </div>
  </section>;
}
