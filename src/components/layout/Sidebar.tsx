import React from 'react';
import { 
  QrCode, 
  Users, 
  FileEdit, 
  FolderSync, 
  FileText, 
  Settings, 
  BookOpen 
} from 'lucide-react';
import { useWindowDrag } from '@/hooks/useWindowDrag';
import logoUrl from '@/assets/logo.svg';

export type NavView = 'login' | 'contacts' | 'drafts' | 'files' | 'summaries' | 'settings';

interface SidebarProps {
  currentView: NavView;
  onSelectView: (view: NavView) => void;
  pendingDraftCount: number;
  onOpenManual: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentView,
  onSelectView,
  pendingDraftCount,
  onOpenManual,
}) => {
  const navItems = [
    { id: 'login' as NavView, label: '账号状态', icon: QrCode },
    { id: 'contacts' as NavView, label: '联系人规则', icon: Users },
    { 
      id: 'drafts' as NavView, 
      label: '草稿审核', 
      icon: FileEdit, 
      badge: pendingDraftCount > 0 ? pendingDraftCount : undefined 
    },
    { id: 'files' as NavView, label: '群文件同步', icon: FolderSync },
    { id: 'summaries' as NavView, label: '智能简报', icon: FileText },
    { id: 'settings' as NavView, label: '系统设置', icon: Settings },
  ];

  const dragHandlers = useWindowDrag();

  return (
    <aside className="w-56 h-screen bg-white border-r border-slate-200/80 flex flex-col justify-between select-none">
      {/* Brand Header (also a window drag region) */}
      <div>
        <div
          data-drag-handle
          {...dragHandlers}
          className="h-14 flex items-center px-4 gap-3 border-b border-slate-200/80 cursor-default"
        >
          <img src={logoUrl} alt="EazyQQ Logo" className="w-7 h-7 shrink-0" />
          <div data-drag-handle className="flex flex-col">
            <span data-drag-handle className="font-semibold tracking-tight text-slate-900 text-sm">
              EazyQQ
            </span>
            <span data-drag-handle className="text-[10px] text-slate-400 font-medium">个人专属助手</span>
          </div>
        </div>

        {/* Main Nav Items */}
        <nav className="p-3 space-y-1">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = currentView === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectView(item.id)}
                className={`w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-sky-50 text-sky-700 font-semibold border border-sky-100 shadow-xs'
                    : 'text-slate-600 hover:text-slate-900 hover:bg-slate-50'
                }`}
              >
                <div className="flex items-center gap-3">
                  <Icon className={`w-4 h-4 ${isActive ? 'text-sky-600' : 'text-slate-400'}`} />
                  <span>{item.label}</span>
                </div>
                {item.badge !== undefined && (
                  <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-amber-500 text-white animate-pulse">
                    {item.badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Footer Manual Action */}
      <div className="p-3 border-t border-slate-100">
        <button
          onClick={onOpenManual}
          className="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm font-medium text-slate-600 hover:text-slate-900 hover:bg-slate-50 transition-colors"
        >
          <div className="flex items-center gap-3">
            <BookOpen className="w-4 h-4 text-slate-400" />
            <span>使用说明书</span>
          </div>
          <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-slate-100 text-slate-500">
            F1
          </span>
        </button>
      </div>
    </aside>
  );
};
