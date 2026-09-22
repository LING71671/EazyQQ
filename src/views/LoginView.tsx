import React, { useState, useEffect } from 'react';
import { QrCode, RefreshCw, Smartphone, CheckCircle, ShieldCheck, Loader2, Sparkles, UserCheck } from 'lucide-react';
import QRCode from 'qrcode';
import type { ProtocolStatusDto, QuickLoginAccountDto } from '@/api/contracts';

interface LoginViewProps {
  status: ProtocolStatusDto;
  onRefreshQr: () => void;
  isLoading: boolean;
  /** Notice or error explaining the state */
  error?: string | null;
  onQuickLogin?: (uin: string) => void;
  isQuickLoggingIn?: string | null;
}

export const LoginView: React.FC<LoginViewProps> = ({
  status,
  onRefreshQr,
  isLoading,
  error,
  onQuickLogin,
  isQuickLoggingIn,
}) => {
  const isLoggedIn = status.loginStatus === 'logged_in';
  const [generatedQr, setGeneratedQr] = useState<string | null>(null);
  const [refreshedNotice, setRefreshedNotice] = useState(false);

  useEffect(() => {
    if (isLoggedIn) {
      setGeneratedQr(null);
      return;
    }
    if (status.qrcodeBase64) {
      if (status.qrcodeBase64.startsWith('http://') || status.qrcodeBase64.startsWith('https://')) {
        QRCode.toDataURL(status.qrcodeBase64, { width: 256, margin: 2 })
          .then((url) => {
            setGeneratedQr(url);
            setRefreshedNotice(true);
            const timer = setTimeout(() => setRefreshedNotice(false), 3000);
            return () => clearTimeout(timer);
          })
          .catch((err) => {
            console.error('Failed to generate QR from URL:', err);
          });
      } else if (status.qrcodeBase64.startsWith('data:')) {
        setGeneratedQr(status.qrcodeBase64);
        setRefreshedNotice(true);
        const timer = setTimeout(() => setRefreshedNotice(false), 3000);
        return () => clearTimeout(timer);
      } else {
        setGeneratedQr(`data:image/png;base64,${status.qrcodeBase64}`);
        setRefreshedNotice(true);
        const timer = setTimeout(() => setRefreshedNotice(false), 3000);
        return () => clearTimeout(timer);
      }
    }
  }, [status.qrcodeBase64, isLoggedIn]);

  const currentQrImage = generatedQr;
  const quickAccounts: QuickLoginAccountDto[] = status.quickLoginAccounts || [];

  // Determine if the current notice is a normal loading/startup state
  const isStartupNotice =
    !currentQrImage ||
    (error && (error.includes('启动') || error.includes('加载') || error.includes('准备') || error.includes('NapCat') || error.includes('凭据')));

  return (
    <div className="flex-1 h-full p-8 flex flex-col items-center justify-center select-none bg-slate-50/50 overflow-y-auto">
      <div className="max-w-lg w-full bg-white rounded-3xl p-8 border border-slate-200/80 shadow-sm flex flex-col items-center text-center transition-all">
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
          <div className="flex flex-col items-center gap-3 py-6">
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
            <div className="mt-2 inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-50 border border-emerald-200/80 text-emerald-700 text-xs font-medium">
              <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              <span>链路畅通，服务已就绪</span>
            </div>
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
                    const isTargetUser = acc.uin === '462564834';
                    return (
                      <div
                        key={acc.uin}
                        className={`flex items-center justify-between p-2.5 rounded-xl border transition-all ${
                          isTargetUser
                            ? 'bg-white border-sky-200/90 shadow-2xs'
                            : 'bg-white/80 border-slate-200/60 hover:border-sky-200'
                        }`}
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
                      <Loader2 className="w-6 h-6 animate-spin text-sky-600" />
                    </div>
                    <span className="text-xs font-medium text-slate-600">正在与底层协议连接…</span>
                    <span className="text-[11px] text-slate-400 leading-tight">若有已记忆账号将自动就绪</span>
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
                ) : error ? (
                  <span className="text-[11px] text-amber-800 font-medium flex items-center gap-1 bg-amber-50 px-3 py-1 rounded-full border border-amber-200/70 shadow-2xs">
                    <RefreshCw className="w-3 h-3 text-amber-600" />
                    尚未加载完成
                  </span>
                ) : (
                  <span className="text-[11px] text-slate-400">
                    若手机提示二维码已过期，请点击下方刷新
                  </span>
                )}

                {error && (
                  <p className="max-w-[18rem] text-[11px] leading-relaxed text-slate-500 text-center mt-1">
                    {error}
                  </p>
                )}
              </div>
            </div>

            {/* Action Button */}
            <button
              onClick={onRefreshQr}
              disabled={isLoading}
              className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-sm transition-all cursor-pointer active:scale-95 disabled:opacity-60 disabled:cursor-not-allowed"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
              <span>{isLoading ? '正在获取全新二维码...' : '刷新二维码'}</span>
            </button>
          </div>
        )}

        {/* Safeguard Note */}
        <div className="mt-6 pt-5 border-t border-slate-100 w-full flex items-center justify-center gap-2 text-slate-400 text-xs">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
          <span>本地优先存储，账号凭证与规则安全保存在本机</span>
        </div>
      </div>
    </div>
  );
};

