import React, { useState, useEffect } from 'react';
import { Sun, Moon, HelpCircle, Minus, Square, Copy, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ProtocolStatusDto } from '@/api/contracts';

interface TopHeaderProps {
  title: string;
  protocolStatus: ProtocolStatusDto;
  isDark: boolean;
  onToggleTheme: () => void;
  onOpenManual: () => void;
}

export const TopHeader: React.FC<TopHeaderProps> = ({
  title,
  protocolStatus,
  isDark,
  onToggleTheme,
  onOpenManual,
}) => {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    try {
      const appWindow = getCurrentWindow();
      appWindow.isMaximized().then(setIsMaximized).catch(() => {});
    } catch {
      // Browser fallback
    }
  }, []);

  const handleMinimize = async () => {
    try {
      const appWindow = getCurrentWindow();
      await appWindow.minimize();
    } catch (e) {
      console.error('Failed to minimize window', e);
    }
  };

  const handleToggleMaximize = async () => {
    try {
      const appWindow = getCurrentWindow();
      await appWindow.toggleMaximize();
      const next = await appWindow.isMaximized();
      setIsMaximized(next);
    } catch (e) {
      console.error('Failed to toggle maximize window', e);
    }
  };

  const handleClose = async () => {
    try {
      const appWindow = getCurrentWindow();
      await appWindow.close();
    } catch (e) {
      console.error('Failed to close window', e);
    }
  };

  const getStatusBadge = () => {
    switch (protocolStatus.loginStatus) {
      case 'logged_in':
        return (
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-50 text-emerald-700 text-xs font-medium border border-emerald-200/60">
            <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span>已登录 ({protocolStatus.nickname || protocolStatus.qqNumber || 'QQ 用户'})</span>
          </div>
        );
      case 'scanned':
        return (
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-amber-50 text-amber-700 text-xs font-medium border border-amber-200/60">
            <span className="w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
            <span>已扫码，等待确认</span>
          </div>
        );
      case 'waiting_scan':
        return (
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-sky-50 text-sky-700 text-xs font-medium border border-sky-200/60">
            <span className="w-2 h-2 rounded-full bg-sky-500" />
            <span>等待手机扫码</span>
          </div>
        );
      default:
        return (
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-slate-100 text-slate-600 text-xs font-medium border border-slate-200">
            <span className="w-2 h-2 rounded-full bg-slate-400" />
            <span>未连接</span>
          </div>
        );
    }
  };

  return (
    <header 
      data-tauri-drag-region 
      className="h-14 bg-white border-b border-slate-200/80 px-4 flex items-center justify-between select-none"
    >
      {/* Title (Draggable region) */}
      <div data-tauri-drag-region className="flex items-center gap-3 flex-1 h-full cursor-default">
        <h1 data-tauri-drag-region className="text-sm font-semibold text-slate-900 tracking-tight">
          {title}
        </h1>
      </div>

      {/* Right Tools & Window Controls */}
      <div className="flex items-center gap-2">
        {/* Status Pill */}
        {getStatusBadge()}

        {/* Theme Toggle */}
        <button
          onClick={onToggleTheme}
          title={isDark ? '切换至明亮模式' : '切换至暗色模式'}
          className="p-1.5 rounded-lg text-slate-500 hover:text-slate-900 hover:bg-slate-100 transition-colors"
        >
          {isDark ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>

        {/* Manual Quick Access */}
        <button
          onClick={onOpenManual}
          title="使用说明书 (F1)"
          className="p-1.5 rounded-lg text-slate-500 hover:text-slate-900 hover:bg-slate-100 transition-colors"
        >
          <HelpCircle className="w-4 h-4" />
        </button>

        {/* Vertical divider */}
        <div className="h-4 w-px bg-slate-200 mx-1" />

        {/* Custom Window Controls */}
        <div className="flex items-center -mr-2">
          <button
            onClick={handleMinimize}
            title="最小化"
            className="w-10 h-10 inline-flex items-center justify-center text-slate-600 hover:text-slate-900 hover:bg-slate-100 transition-colors"
          >
            <Minus className="w-3.5 h-3.5 stroke-[1.75]" />
          </button>
          <button
            onClick={handleToggleMaximize}
            title={isMaximized ? '还原' : '最大化'}
            className="w-10 h-10 inline-flex items-center justify-center text-slate-600 hover:text-slate-900 hover:bg-slate-100 transition-colors"
          >
            {isMaximized ? (
              <Copy className="w-3.5 h-3.5 stroke-[1.75]" />
            ) : (
              <Square className="w-3.5 h-3.5 stroke-[1.75]" />
            )}
          </button>
          <button
            onClick={handleClose}
            title="关闭"
            className="w-10 h-10 inline-flex items-center justify-center text-slate-600 hover:text-white hover:bg-red-500 transition-colors"
          >
            <X className="w-4 h-4 stroke-[1.75]" />
          </button>
        </div>
      </div>
    </header>
  );
};
