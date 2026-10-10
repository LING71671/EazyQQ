import { AccountLoginQr } from '@/components/accounts/AccountLoginQr';
import { AccountManager } from '@/components/accounts/AccountManager';
import React, { useState, useEffect } from 'react';
import { RefreshCw, CheckCircle, Loader2, Activity, LogOut, AlertCircle } from 'lucide-react';
import { QuickLoginAccounts } from '@/components/accounts/QuickLoginAccounts';
import QRCode from 'qrcode';
import type { ProtocolStatusDto, QuickLoginAccountDto } from '@/api/contracts';

interface LoginViewProps {
  pendingLogin?: { uin: string; qrcodeBase64: string } | null;
  onCancelPendingLogin?: () => void;
  status: ProtocolStatusDto;
  onRefreshQr: () => void;
  onRestoreProtocol?: () => void;
  isLoading: boolean;
  /** Notice or error explaining the state */
  error?: string | null;
  onQuickLogin?: (uin: string) => void;
  isQuickLoggingIn?: string | null;
  onOpenHealth?: () => void;
  onLogout?: () => void;
}

export const LoginView: React.FC<LoginViewProps> = ({
  status,
  pendingLogin,
  onCancelPendingLogin,
  onRefreshQr,
  onRestoreProtocol,
  isLoading,
  error,
  onQuickLogin,
  isQuickLoggingIn,
  onOpenHealth,
  onLogout,
}) => {
  const isLoggedIn = status.loginStatus === 'logged_in';
  const [generatedQr, setGeneratedQr] = useState<string | null>(null);
  const [refreshedNotice, setRefreshedNotice] = useState(false);
  const [waitExpired, setWaitExpired] = useState(false);
  useEffect(() => {
    setWaitExpired(false);
    if (status.qrcodeBase64 || isLoggedIn) return;
    const timer = setTimeout(() => setWaitExpired(true), 20000);
    return () => clearTimeout(timer);
  }, [status.qrcodeBase64, isLoggedIn, isLoading]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    setGeneratedQr(null);
    if (!isLoggedIn && status.qrcodeBase64) {
      const raw = status.qrcodeBase64;
      const image = raw.startsWith('http://') || raw.startsWith('https://')
        ? QRCode.toDataURL(raw, { width: 256, margin: 2 })
        : Promise.resolve(raw.startsWith('data:') ? raw : `data:image/png;base64,${raw}`);
      image.then(url => {
        if (cancelled) return;
        setGeneratedQr(url); setRefreshedNotice(true);
        timer = setTimeout(() => setRefreshedNotice(false), 3000);
      }).catch(() => { if (!cancelled) setGeneratedQr(null); });
    }
    return () => { cancelled = true; clearTimeout(timer); };
  }, [status.qrcodeBase64, isLoggedIn]);

  const currentQrImage = generatedQr;
  const quickAccounts: QuickLoginAccountDto[] = status.quickLoginAccounts || [];
  const otherQuickAccounts = quickAccounts.filter((acc) => acc.uin !== status.qqNumber);

  const displayError = error || status.qrcodeError || (waitExpired && !currentQrImage ? '二维码等待超过 20 秒，请查看链路诊断。' : null);


  return (
    <div className="@container h-full overflow-y-auto bg-slate-50/50 p-4 sm:p-6" data-login-workspace>
      <div className="mx-auto grid w-full max-w-5xl items-start gap-6 @min-[600px]:grid-cols-[minmax(240px,0.8fr)_minmax(0,1fr)]">
        <div className="min-w-0">
          {pendingLogin ? (
            <AccountLoginQr pending={pendingLogin} onConfirmed={uin => onQuickLogin?.(uin)} onCancel={() => onCancelPendingLogin?.()} />
          ) : (
            <section aria-label={isLoggedIn ? '当前 QQ 会话' : '扫码登录'} className="rounded-2xl border border-slate-200 bg-white p-5 text-center dark:border-slate-700 dark:bg-slate-900" data-login-primary>
              <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">{isLoggedIn ? '当前 QQ 会话' : '扫码登录'}</h2>
              {isLoggedIn ? (
                <div className="mt-4 flex flex-col items-center gap-3">
                  {status.avatarUrl ? <img src={status.avatarUrl} alt="当前账号头像" className="h-16 w-16 rounded-full object-cover" /> : <CheckCircle className="h-12 w-12 text-emerald-600" />}
                  <div className="min-w-0 w-full"><p className="break-words font-semibold text-slate-900 dark:text-slate-100">{status.nickname || 'QQ 账号'}</p><p className="mt-1 text-sm tabular-nums text-slate-600 dark:text-slate-400">{status.qqNumber || '已连接'}</p></div>
                  <p role="status" className="text-sm font-medium text-emerald-700 dark:text-emerald-400">QQ 会话已登录</p>
                  {error && <p role="alert" className="break-words text-sm text-red-700 dark:text-red-400">{error}</p>}
                  {onLogout && <button type="button" onClick={onLogout} className="inline-flex items-center justify-center gap-2 rounded-lg border border-slate-300 px-3 py-2 text-sm text-rose-700 hover:bg-rose-50 focus-visible:outline-2 focus-visible:outline-sky-600 dark:border-slate-700 dark:text-rose-400"><LogOut className="h-4 w-4" />退出当前账号</button>}
                  {onOpenHealth && <button type="button" onClick={onOpenHealth} className="rounded-lg px-3 py-2 text-sm text-slate-700 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-sky-600 dark:text-slate-300">链路体检与自愈</button>}
                </div>
              ) : (
                <>
                  <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">使用手机 QQ 扫码确认</p>
                  <div className="relative mx-auto mt-4 flex aspect-square w-full max-w-56 items-center justify-center overflow-hidden rounded-lg border border-slate-200 bg-white p-2" aria-busy={isLoading}>
                    {currentQrImage ? <img src={currentQrImage} alt="Login QR Code" width={256} height={256} className="h-full w-full object-contain" /> : (
                      <div className="flex flex-col items-center gap-3 p-3 text-sm text-slate-600">
                        {displayError && !isLoading ? <AlertCircle className="h-6 w-6 text-amber-600" /> : <Loader2 className="h-6 w-6 animate-spin text-sky-600" />}
                        <span>{displayError && !isLoading ? '二维码服务不可用' : '正在连接二维码服务…'}</span>
                      </div>
                    )}
                  </div>
                  <div className="mt-3 text-sm" role="status" aria-live="polite">
                    {displayError ? <p role="alert" className="break-words text-amber-800 dark:text-amber-300">{displayError}</p> : currentQrImage ? <p className="text-slate-600 dark:text-slate-400">{status.loginStatus === 'scanned' ? '已扫码，请在手机确认' : refreshedNotice ? '最新有效二维码已就绪' : '等待手机扫码'}</p> : <p className="text-slate-600 dark:text-slate-400">服务加载中</p>}
                  </div>
                  <div className="mt-4 flex flex-wrap justify-center gap-2">
                    <button type="button" onClick={!status.isConnected && displayError && onRestoreProtocol ? onRestoreProtocol : onRefreshQr} disabled={isLoading} className="inline-flex items-center justify-center gap-2 rounded-lg bg-sky-600 px-3 py-2 text-sm font-medium text-white hover:bg-sky-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:cursor-not-allowed disabled:opacity-60"><RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />{isLoading ? '正在处理…' : !status.isConnected && displayError && onRestoreProtocol ? '恢复协议' : '刷新二维码'}</button>
                    {onOpenHealth && <button type="button" onClick={onOpenHealth} className="inline-flex items-center justify-center gap-2 rounded-lg border border-slate-300 px-3 py-2 text-sm text-slate-700 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-sky-600 dark:border-slate-700 dark:text-slate-300"><Activity className="h-4 w-4" />链路体检与自愈</button>}
                  </div>
                </>
              )}
            </section>
          )}
        </div>
        <div className="min-w-0 space-y-5">
          <QuickLoginAccounts accounts={isLoggedIn ? otherQuickAccounts : quickAccounts} loggedIn={isLoggedIn} onLogin={onQuickLogin} switching={isQuickLoggingIn} />
          <details className="rounded-xl border border-slate-200 bg-white dark:border-slate-700 dark:bg-slate-900" data-account-management>
            <summary className="cursor-pointer rounded-xl px-4 py-3 text-sm font-medium text-slate-900 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-sky-600 dark:text-slate-100 dark:hover:bg-slate-800">账号管理与批量操作</summary>
            <AccountManager onSelect={onQuickLogin} switching={isQuickLoggingIn} />
          </details>
        </div>
      </div>
    </div>
  );
};
