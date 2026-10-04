import React, { useState, useEffect, useCallback } from 'react';
import { Sun, Moon, HelpCircle, Minus, Square, Copy, X, Activity } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from '@/api/client';
import { useWindowDrag } from '@/hooks/useWindowDrag';
import type { ProtocolStatusDto } from '@/api/contracts';

interface TopHeaderProps {
  title: string;
  protocolStatus: ProtocolStatusDto;
  isDark: boolean;
  onToggleTheme: () => void;
  onOpenManual: () => void;
  onOpenHealth?: () => void;
  chainHasFailure?: boolean;
}

export const TopHeader: React.FC<TopHeaderProps> = ({
  title,
  protocolStatus,
  isDark,
  onToggleTheme,
  onOpenManual,
  onOpenHealth,
  chainHasFailure = false,
}) => {
  const [isMaximized, setIsMaximized] = useState(false);
  const [minimizeToTray, setMinimizeToTray] = useState(true);
  const [closeToTray, setCloseToTray] = useState(true);

  // Read the persisted window behavior so button tooltips match reality.
  useEffect(() => {
    api
      .getWindowBehavior()
      .then((res) => {
        if (res.success && res.data) {
          setMinimizeToTray(res.data.minimizeToTray);
          setCloseToTray(res.data.closeToTray);
        }
      })
      .catch(() => {});
  }, []);

  // Keep the maximize/restore icon in sync with the native window state.
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const syncMaximized = () => {
      try {
        getCurrentWindow().isMaximized().then(setIsMaximized).catch(() => {});
      } catch {
        // Not running inside a Tauri webview (browser preview) - ignore.
      }
    };

    syncMaximized();

    try {
      getCurrentWindow()
        .onResized(() => syncMaximized())
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => {});
    } catch {
      // Browser fallback
    }

    return () => unlisten?.();
  }, []);

  const handleMinimize = useCallback(async () => {
    try {
      await api.minimizeWindow();
    } catch (e) {
      console.error('Failed to minimize window', e);
    }
  }, []);

  const handleToggleMaximize = useCallback(async () => {
    try {
      const res = await api.toggleMaximizeWindow();
      if (res.success && typeof res.data === 'boolean') {
        setIsMaximized(res.data);
      }
    } catch (e) {
      console.error('Failed to toggle maximize window', e);
    }
  }, []);

  const handleClose = useCallback(async () => {
    try {
      await api.closeWindow();
    } catch (e) {
      console.error('Failed to close window', e);
    }
  }, []);

  // Frameless-window dragging, routed through a native command (see useWindowDrag).
  const { onMouseDown: handleDragMouseDown, onDoubleClick: handleDragDoubleClick } =
    useWindowDrag(handleToggleMaximize);

  const getStatusBadge = () => {
    if (chainHasFailure) {
      return (
        <button
          onClick={onOpenHealth}
          title="检测到链路存在异常，点击查看体检与一键自愈"
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-amber-50 text-amber-800 text-xs font-medium border border-amber-300 hover:bg-amber-100 transition-all cursor-pointer shadow-2xs animate-pulse"
        >
          <span className="w-2 h-2 rounded-full bg-amber-500" />
          <span>链路异常 · 点击修复</span>
        </button>
      );
    }

    switch (protocolStatus.loginStatus) {
      case 'logged_in':
        return (
          <button
            onClick={onOpenHealth}
            title="点击查看全链路体检"
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-50 text-emerald-700 text-xs font-medium border border-emerald-200/60 hover:bg-emerald-100 transition-all cursor-pointer"
          >
            <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span>已登录 ({protocolStatus.nickname || protocolStatus.qqNumber || 'QQ 用户'})</span>
          </button>
        );
      case 'scanned':
        return (
          <button
            onClick={onOpenHealth}
            title="点击查看全链路体检"
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-amber-50 text-amber-700 text-xs font-medium border border-amber-200/60 hover:bg-amber-100 transition-all cursor-pointer"
          >
            <span className="w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
            <span>已扫码，等待确认</span>
          </button>
        );
      case 'waiting_scan':
        return (
          <button
            onClick={onOpenHealth}
            title="点击查看全链路体检"
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-sky-50 text-sky-700 text-xs font-medium border border-sky-200/60 hover:bg-sky-100 transition-all cursor-pointer"
          >
            <span className="w-2 h-2 rounded-full bg-sky-500" />
            <span>等待手机扫码</span>
          </button>
        );
      default:
        return (
          <button
            onClick={onOpenHealth}
            title="点击排查连接状态"
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-slate-100 text-slate-600 text-xs font-medium border border-slate-200 hover:bg-slate-200 transition-all cursor-pointer"
          >
            <span className="w-2 h-2 rounded-full bg-slate-400" />
            <span>未连接 · 点击排障</span>
          </button>
        );
    }
  };

  return (
    <header
      data-drag-handle
      onMouseDown={handleDragMouseDown}
      onDoubleClick={handleDragDoubleClick}
      className="h-14 shrink-0 bg-white border-b border-slate-200/80 px-4 flex items-center justify-between select-none"
    >
      {/* Title (Draggable region) */}
      <div data-drag-handle className="flex items-center gap-3 flex-1 h-full cursor-default">
        <h1 data-drag-handle className="text-sm font-semibold text-slate-900 tracking-tight">
          {title}
        </h1>
      </div>

      {/* Right Tools & Window Controls */}
      <div className="flex items-center gap-2" data-no-drag>
        {/* Status Pill */}
        {getStatusBadge()}

        {/* Health Check Quick Access */}
        <button
          onClick={onOpenHealth}
          title="链路健康体检 & 自愈中心"
          className="p-1.5 rounded-lg text-slate-500 hover:text-sky-600 hover:bg-sky-50 transition-colors"
        >
          <Activity className="w-4 h-4" />
        </button>

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
            title={minimizeToTray ? '最小化到系统托盘' : '最小化'}
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
            title={closeToTray ? '关闭窗口并缩至托盘 (后台继续运行)' : '退出 EazyQQ'}
            className="w-10 h-10 inline-flex items-center justify-center text-slate-600 hover:text-white hover:bg-red-500 transition-colors"
          >
            <X className="w-4 h-4 stroke-[1.75]" />
          </button>
        </div>
      </div>
    </header>
  );
};
