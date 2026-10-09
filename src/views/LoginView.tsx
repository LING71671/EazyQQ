import { AccountLoginQr } from '@/components/accounts/AccountLoginQr';
import { AccountManager } from '@/components/accounts/AccountManager';
import React, { useState, useEffect } from 'react';
import { QrCode, RefreshCw, Smartphone, CheckCircle, ShieldCheck, Loader2, Sparkles, UserCheck, Activity, LogOut, ArrowRightLeft, AlertCircle } from 'lucide-react';
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
  const isStartupNotice = !currentQrImage && !displayError;

  return (
    <div className="flex-1 h-full p-8 flex flex-col items-center select-none bg-slate-50/50 overflow-y-auto">
      <div className="max-w-lg w-full bg-white rounded-3xl p-8 shrink-0 border border-slate-200/80 shadow-sm flex flex-col items-center text-center transition-all">
        {/* Header Icon */}
        <div className="w-12 h-12 rounded-2xl bg-sky-50 text-sky-600 flex items-center justify-center mb-4 border border-sky-100 shadow-2xs">
          {isLoggedIn ? <CheckCircle className="w-6 h-6 text-emerald-600" /> : <QrCode className="w-6 h-6" />}
        </div>

        <h2 className="text-xl font-bold text-slate-900 tracking-tight mb-1">
          {isLoggedIn ? 'QQ 已成功连接' : '登录个人 QQ'}
        </h2>
        <p className="text-xs text-slate-500 mb-6">
          {isLoggedIn
            ? '智能助手正在后台待命，监听并处理群聊与私聊会话'
            : quickAccounts.length > 0
            ? '检测到本机已记住的账号，可免扫码一键快速登录'
            : '打开手机 QQ 扫一扫二维码，授权一键登录'}
        </p>

        {/* Logged in state */}
        {isLoggedIn ? (
          <div className="w-full flex flex-col items-center gap-4 py-4">
            <div className="flex flex-col items-center gap-2.5">
              <div className="w-20 h-20 rounded-full border-2 border-emerald-500/40 p-1 bg-emerald-50 flex items-center justify-center shadow-xs">
                {status.avatarUrl ? (
                  <img src={status.avatarUrl} alt="Avatar" className="w-full h-full rounded-full object-cover" />
                ) : (
                  <CheckCircle className="w-10 h-10 text-emerald-600" />
                )}
              </div>
              <div className="text-sm font-semibold text-slate-900">
                {status.nickname || 'QQ 账号'}
              </div>
              <span className="text-xs text-slate-400 font-mono">
                QQ: {status.qqNumber || '已连接'}
              </span>
              <div className="mt-1 inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-50 border border-emerald-200/80 text-emerald-700 text-xs font-medium">
                <span className="w-2 h-2 rounded-full bg-emerald-500" />
                <span>QQ 会话已登录</span>
              </div>
              {error && (
                <div className="mt-2 w-full p-2.5 rounded-xl bg-red-50 border border-red-200 text-xs text-red-700 text-center animate-in fade-in">
                  {error}
                </div>
              )}
            </div>

            {/* Other quick login accounts available for switching */}
            {otherQuickAccounts.length > 0 && (
              <div className="w-full mt-3 p-4 rounded-2xl bg-sky-50/50 border border-sky-100/90 text-left">
                <div className="flex items-center gap-2 mb-2.5 text-xs font-semibold text-sky-900">
                  <ArrowRightLeft className="w-3.5 h-3.5 text-sky-600" />
                  <span>切换至本机其他已记忆账号</span>
                </div>
                <div className="space-y-2">
                  {otherQuickAccounts.map((acc) => {
                    const isLoggingThis = isQuickLoggingIn === acc.uin;
                    return (
                      <div
                        key={acc.uin}
                        className="flex items-center justify-between p-2.5 rounded-xl border bg-white border-slate-200/80 hover:border-sky-200 transition-all"
                      >
                        <div className="flex items-center gap-2.5 min-w-0">
                          <img
                            src={acc.faceUrl || `https://q1.qlogo.cn/g?b=qq&nk=${acc.uin}&s=100`}
                            alt={acc.nickname}
                            className="w-8 h-8 rounded-full border border-slate-100 object-cover shrink-0"
                            onError={(e) => {
                              (e.target as HTMLElement).style.display = 'none';
                            }}
                          />
                          <div className="flex flex-col min-w-0">
                            <span className="text-xs font-semibold text-slate-800 truncate">
                              {acc.nickname}
                            </span>
                            <span className="text-[11px] text-slate-400 font-mono">
                              {acc.uin}
                            </span>
                          </div>
                        </div>

                        <button
                          onClick={() => onQuickLogin && onQuickLogin(acc.uin)}
                          disabled={!!isQuickLoggingIn}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-medium shadow-2xs transition-all cursor-pointer disabled:opacity-60"
                        >
                          {isLoggingThis ? (
                            <>
                              <Loader2 className="w-3.5 h-3.5 animate-spin" />
                              <span>切换中…</span>
                            </>
                          ) : (
                            <>
                              <ArrowRightLeft className="w-3 h-3" />
                              <span>切换登录</span>
                            </>
                          )}
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Logout / Switch action */}
            {onLogout && (
              <button
                type="button"
                onClick={onLogout}
                className="mt-2 flex items-center gap-1.5 px-4 py-2 rounded-xl border border-slate-200 hover:border-rose-300 hover:bg-rose-50/50 text-rose-700 hover:text-rose-700 text-xs font-medium transition-all cursor-pointer"
              >
                <LogOut className="w-3.5 h-3.5" />
                <span>退出当前账号 / 重新扫码登录</span>
              </button>
            )}
          </div>
        ) : (
          <div className="w-full flex flex-col items-center">
            {/* Quick Login Accounts (If available) */}
            {quickAccounts.length > 0 && (
              <div className="w-full mb-6 p-4 rounded-2xl bg-sky-50/50 border border-sky-100/90 text-left">
                <div className="flex items-center gap-2 mb-3 text-xs font-semibold text-sky-900">
                  <UserCheck className="w-4 h-4 text-sky-600" />
                  <span>本机已记忆账号（免扫码直接登录）</span>
                </div>
                <div className="space-y-2">
                  {quickAccounts.map((acc) => {
                    const isLoggingThis = isQuickLoggingIn === acc.uin;
                    return (
                      <div
                        key={acc.uin}
                        className="flex items-center justify-between p-2.5 rounded-xl border bg-white border-slate-200/80 hover:border-sky-200 transition-all shadow-2xs"
                      >
                        <div className="flex items-center gap-2.5 min-w-0">
                          <img
                            src={acc.faceUrl || `https://q1.qlogo.cn/g?b=qq&nk=${acc.uin}&s=100`}
                            alt={acc.nickname}
                            className="w-8 h-8 rounded-full border border-slate-100 object-cover shrink-0"
                            onError={(e) => {
                              (e.target as HTMLElement).style.display = 'none';
                            }}
                          />
                          <div className="flex flex-col min-w-0">
                            <span className="text-xs font-semibold text-slate-800 truncate">
                              {acc.nickname}
                            </span>
                            <span className="text-[11px] text-slate-400 font-mono">
                              {acc.uin}
                            </span>
                          </div>
                        </div>

                        <button
                          onClick={() => onQuickLogin && onQuickLogin(acc.uin)}
                          disabled={!!isQuickLoggingIn}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-medium shadow-2xs transition-all cursor-pointer active:scale-95 disabled:opacity-60"
                        >
                          {isLoggingThis ? (
                            <>
                              <Loader2 className="w-3.5 h-3.5 animate-spin" />
                              <span>登录中…</span>
                            </>
                          ) : (
                            <>
                              <Sparkles className="w-3.5 h-3.5" />
                              <span>一键登录</span>
                            </>
                          )}
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* QR Code Container */}
            <div className="flex flex-col items-center gap-3 mb-6">
              <div className="relative group p-3 rounded-2xl bg-white border-2 border-sky-100/80 shadow-sm flex items-center justify-center w-56 h-56 overflow-hidden">
                {isLoading && (
                  <div className="absolute top-0 inset-x-0 h-1 bg-gradient-to-r from-sky-400 via-sky-500 to-sky-600 animate-pulse z-10" />
                )}

                {currentQrImage ? (
                  <img
                    src={currentQrImage}
                    alt="Login QR Code"
                    className="w-full h-full object-contain rounded-xl transition-all duration-300"
                  />
                ) : (
                  <div className="flex flex-col items-center gap-3 text-slate-400 p-4">
                    <div className="w-12 h-12 rounded-full bg-sky-50 flex items-center justify-center text-sky-500">
                      {displayError && !isLoading ? <AlertCircle className="w-6 h-6 text-amber-600" /> : <Loader2 className="w-6 h-6 animate-spin text-sky-600" />}
                    </div>
                    <span className="text-sm font-medium text-slate-600">{displayError && !isLoading ? '二维码服务不可用' : '正在连接二维码服务…'}</span>
                    {displayError && !isLoading && <span className="text-xs text-slate-500 leading-tight">查看诊断或恢复协议</span>}
                  </div>
                )}
              </div>

              {/* Status Notice - Calm, non-anxious design */}
              <div className="flex flex-col items-center gap-1">
                {refreshedNotice ? (
                  <span className="text-[11px] text-emerald-700 font-medium flex items-center gap-1 bg-emerald-50 px-3 py-1 rounded-full border border-emerald-200/70 shadow-2xs">
                    <CheckCircle className="w-3 h-3 text-emerald-600" />
                    最新有效二维码已就绪
                  </span>
                ) : isStartupNotice ? (
                  <span className="text-[11px] text-sky-700 font-medium flex items-center gap-1.5 bg-sky-50 px-3 py-1 rounded-full border border-sky-200/70 shadow-2xs">
                    <Loader2 className="w-3 h-3 animate-spin text-sky-600" />
                    服务加载中
                  </span>
                ) : displayError ? (
                  <span className="text-[11px] text-amber-800 font-medium flex items-center gap-1 bg-amber-50 px-3 py-1 rounded-full border border-amber-200/70 shadow-2xs">
                    <RefreshCw className="w-3 h-3 text-amber-600" />
                    连接失败
                  </span>
                ) : (
                  <span className="text-[11px] text-slate-400">
                    若手机提示二维码已过期，请点击下方刷新
                  </span>
                )}

                {displayError && (
                  <p role="alert" className="max-w-[24rem] text-sm leading-relaxed text-slate-600 text-center mt-1 break-words">
                    {displayError}
                  </p>
                )}
              </div>
            </div>

            {/* Action Buttons */}
            <div className="flex items-center gap-3">
              <button
                onClick={!status.isConnected && displayError && onRestoreProtocol ? onRestoreProtocol : onRefreshQr}
                disabled={isLoading}
                className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-sm transition-all cursor-pointer active:scale-95 disabled:opacity-60 disabled:cursor-not-allowed"
              >
                <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
                <span>{isLoading ? '正在处理…' : !status.isConnected && displayError && onRestoreProtocol ? '恢复协议' : '刷新二维码'}</span>
              </button>

              {onOpenHealth && (
                <button
                  type="button"
                  onClick={onOpenHealth}
                  className="flex items-center gap-1.5 px-3.5 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-medium transition-all cursor-pointer"
                >
                  <Activity className="w-3.5 h-3.5 text-slate-500" />
                  <span>链路体检与自愈</span>
                </button>
              )}
            </div>
          </div>
        )}

        {/* Safeguard Note */}
        <div className="mt-6 pt-5 border-t border-slate-100 w-full flex items-center justify-center gap-2 text-slate-400 text-xs">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
          <span>本地优先存储，账号凭证与规则安全保存在本机</span>
        </div>
      </div>
      {pendingLogin && <AccountLoginQr pending={pendingLogin} onConfirmed={uin => onQuickLogin?.(uin)} onCancel={() => onCancelPendingLogin?.()} />}
      <AccountManager onSelect={onQuickLogin} switching={isQuickLoggingIn} />
    </div>
  );
};

