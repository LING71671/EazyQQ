import React from 'react';
import { Settings, ShieldAlert } from 'lucide-react';

interface WindowBehaviorCardProps {
  minimizeToTray: boolean;
  closeToTray: boolean;
  onChangeBehavior: (behavior: { minimizeToTray?: boolean; closeToTray?: boolean }) => void;
}

export const WindowBehaviorCard: React.FC<WindowBehaviorCardProps> = ({
  minimizeToTray,
  closeToTray,
  onChangeBehavior,
}) => {
  return (
    <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-4">
      <div className="flex items-center justify-between pb-2 border-b border-slate-100">
        <span className="text-sm font-semibold text-slate-900 flex items-center gap-2">
          <Settings className="w-4 h-4 text-sky-600" />
          <span>窗口与系统托盘行为</span>
        </span>
        <span className="text-[11px] text-slate-400">即改即生效，无需重启</span>
      </div>

      <div className="space-y-3 text-xs">
        <div className="p-3 rounded-xl bg-sky-50/70 border border-sky-100 flex items-start gap-2.5">
          <ShieldAlert className="w-4 h-4 text-sky-600 shrink-0 mt-0.5" />
          <div className="text-sky-900 leading-relaxed text-[11px]">
            <strong className="font-semibold block">托盘常驻保护：</strong>
            窗口缩入托盘后，QQ 协议监听、消息接管与定时群总结仍在后台持续运行，不会被中断。
          </div>
        </div>

        <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50 border border-slate-100">
          <div className="pr-4">
            <span className="font-semibold block text-slate-800">最小化时缩至系统托盘</span>
            <span className="text-slate-400 text-[11px]">
              点击最小化按钮时隐藏窗口至右下角托盘，而非保留在任务栏
            </span>
          </div>
          <input
            type="checkbox"
            checked={minimizeToTray}
            onChange={(e) => onChangeBehavior({ minimizeToTray: e.target.checked })}
            className="w-4 h-4 shrink-0 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
          />
        </div>

        <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50 border border-slate-100">
          <div className="pr-4">
            <span className="font-semibold block text-slate-800">关闭时缩至系统托盘</span>
            <span className="text-slate-400 text-[11px]">
              点击关闭按钮仅隐藏窗口，不退出进程；彻底退出请右键托盘图标选择「退出 EazyQQ」
            </span>
          </div>
          <input
            type="checkbox"
            checked={closeToTray}
            onChange={(e) => onChangeBehavior({ closeToTray: e.target.checked })}
            className="w-4 h-4 shrink-0 text-sky-600 rounded border-slate-300 focus:ring-sky-500 cursor-pointer accent-sky-600"
          />
        </div>

        <div className="pt-1 text-[11px] text-slate-400 leading-relaxed">
          标题栏空白处可按住拖动窗口，双击标题栏可最大化 / 还原；单击托盘图标即可重新呼出主窗口。
        </div>
      </div>
    </div>
  );
};
