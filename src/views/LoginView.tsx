import React, { useState, useEffect } from 'react';
import { QrCode, RefreshCw, Smartphone, CheckCircle, ShieldCheck, AlertCircle } from 'lucide-react';
import QRCode from 'qrcode';
import type { ProtocolStatusDto } from '@/api/contracts';

interface LoginViewProps {
  status: ProtocolStatusDto;
  onRefreshQr: () => void;
  isLoading: boolean;
  /** Why the last refresh attempt failed, if it did. */
  error?: string | null;
}

export const LoginView: React.FC<LoginViewProps> = ({ status, onRefreshQr, isLoading, error }) => {
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
          .then(url => {
            setGeneratedQr(url);
            setRefreshedNotice(true);
            const timer = setTimeout(() => setRefreshedNotice(false), 3000);
            return () => clearTimeout(timer);
          })
          .catch(err => {
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

  const handleRefresh = () => {
    onRefreshQr();
  };

  const currentQrImage = generatedQr;

  return (
    <div className="flex-1 h-full p-8 flex flex-col items-center justify-center select-none bg-slate-50/60">
      <div className="max-w-md w-full bg-white rounded-2xl p-8 border border-slate-200/80 shadow-sm flex flex-col items-center text-center">
        {/* Header Icon */}
        <div className="w-12 h-12 rounded-2xl bg-sky-50 text-sky-600 flex items-center justify-center mb-4 border border-sky-100">
          <QrCode className="w-6 h-6" />
        </div>

        <h2 className="text-xl font-bold text-slate-900 tracking-tight mb-1">
          {isLoggedIn ? 'QQ 已成功连接' : '扫码登录个人 QQ'}
        </h2>
        <p className="text-xs text-slate-500 mb-6">
          {isLoggedIn 
            ? '智能助手正在后台待命，监听并处理会话' 
            : '打开手机 QQ 扫一扫下方二维码，授权一键登录'}
        </p>

        {/* QR Code Container */}
        {isLoggedIn ? (
          <div className="flex flex-col items-center gap-3 py-6">
            <div className="w-20 h-20 rounded-full border-2 border-emerald-500/40 p-1 bg-emerald-50 flex items-center justify-center">
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
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3 mb-6">
            <div className="relative group p-3 rounded-2xl bg-white border-2 border-sky-100 shadow-sm flex items-center justify-center w-56 h-56 overflow-hidden">
              {/* Subtle top progress bar when loading without any white-out */}
              {isLoading && (
                <div className="absolute top-0 inset-x-0 h-1 bg-gradient-to-r from-sky-400 via-sky-500 to-sky-600 animate-pulse z-10" />
              )}

              {currentQrImage ? (
                <img 
                  src={currentQrImage} 
                  alt="Login QR Code" 
                  className={`w-full h-full object-contain rounded-xl transition-opacity ${error ? 'opacity-40' : ''}`}
                />
              ) : (
                <div className="flex flex-col items-center gap-2 text-slate-400">
                  <Smartphone className="w-8 h-8 animate-bounce text-sky-500" />
                  <span className="text-xs">正在向腾讯请求二维码...</span>
                </div>
              )}

              {/* A failed refresh leaves the previous code on screen. Say so on the image
                  itself, otherwise it looks like the button simply did nothing. */}
              {error && currentQrImage && (
                <div className="absolute inset-0 flex items-center justify-center z-20">
                  <span className="text-[11px] font-semibold text-rose-700 bg-rose-50/95 px-3 py-1.5 rounded-lg border border-rose-200">
                    此二维码可能已过期
                  </span>
                </div>
              )}
            </div>

            {/* Status notice */}
            <div className="h-5 flex items-center justify-center">
              {error ? (
                <span className="text-[11px] text-rose-600 font-medium flex items-center gap-1 bg-rose-50 px-2.5 py-0.5 rounded-full border border-rose-200/60">
                  <AlertCircle className="w-3 h-3 text-rose-500" />
                  刷新失败
                </span>
              ) : refreshedNotice ? (
                <span className="text-[11px] text-emerald-600 font-medium flex items-center gap-1 bg-emerald-50 px-2.5 py-0.5 rounded-full border border-emerald-200/60">
                  <CheckCircle className="w-3 h-3 text-emerald-500" />
                  最新有效二维码已就绪
                </span>
              ) : (
                <span className="text-[11px] text-slate-400">
                  若手机提示二维码已过期，请点击下方刷新
                </span>
              )}
            </div>

            {/* The reason, verbatim. The backend already explains it (protocol side down,
                QQ path wrong); hiding that behind "刷新失败" would waste it. */}
            {error && (
              <p className="max-w-[16rem] text-[11px] leading-relaxed text-rose-600/90 text-center break-words">
                {error}
              </p>
            )}
          </div>
        )}

        {/* Action Button */}
        {!isLoggedIn && (
          <button
            onClick={handleRefresh}
            disabled={isLoading}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-sky-600 hover:bg-sky-700 active:bg-sky-800 text-white text-xs font-semibold shadow-sm transition-all disabled:opacity-60 disabled:cursor-not-allowed"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>{isLoading ? '正在获取全新二维码...' : '刷新二维码'}</span>
          </button>
        )}

        {/* Safeguard Note */}
        <div className="mt-6 pt-5 border-t border-slate-100 w-full flex items-center justify-center gap-2 text-slate-400 text-xs">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
          <span>本地优先存储，账号凭证安全保存在本机</span>
        </div>
      </div>
    </div>
  );
};
