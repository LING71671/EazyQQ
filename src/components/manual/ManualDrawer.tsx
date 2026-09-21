import React, { useState } from 'react';
import { X, BookOpen, Cpu, ShieldCheck, FolderSync, Keyboard, HelpCircle } from 'lucide-react';

interface ManualDrawerProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ManualDrawer: React.FC<ManualDrawerProps> = ({ isOpen, onClose }) => {
  const [activeTab, setActiveTab] = useState<'routing' | 'opencode' | 'files' | 'shortcuts' | 'faq'>('routing');

  if (!isOpen) return null;

  const chapters = [
    { id: 'routing', label: '1. 四象限策略说明', icon: ShieldCheck },
    { id: 'opencode', label: '2. 本地 OpenCode 零配置', icon: Cpu },
    { id: 'files', label: '3. 群文件与本地知识库', icon: FolderSync },
    { id: 'shortcuts', label: '4. 快捷键与常用技巧', icon: Keyboard },
    { id: 'faq', label: '5. 故障排查与一键自检', icon: HelpCircle },
  ] as const;

  return (
    <div className="fixed inset-0 z-50 overflow-hidden select-none">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-slate-900/30 backdrop-blur-xs transition-opacity"
        onClick={onClose} 
      />

      {/* Drawer Panel */}
      <div className="absolute inset-y-0 right-0 max-w-2xl w-full bg-white shadow-2xl border-l border-slate-200 flex flex-col">
        {/* Header */}
        <div className="h-14 px-6 border-b border-slate-100 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <BookOpen className="w-5 h-5 text-sky-600" />
            <span className="font-semibold text-slate-900 text-sm">EazyQQ 使用说明书</span>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-600 hover:bg-slate-100 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content Layout */}
        <div className="flex-1 flex overflow-hidden">
          {/* Chapter Tabs */}
          <div className="w-52 border-r border-slate-100 p-3 space-y-1">
            {chapters.map((ch) => {
              const Icon = ch.icon;
              const isActive = activeTab === ch.id;
              return (
                <button
                  key={ch.id}
                  onClick={() => setActiveTab(ch.id)}
                  className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium text-left transition-colors ${
                    isActive
                      ? 'bg-sky-50 text-sky-700 font-semibold border border-sky-100 shadow-xs'
                      : 'text-slate-600 hover:bg-slate-50'
                  }`}
                >
                  <Icon className="w-3.5 h-3.5 shrink-0" />
                  <span className="truncate">{ch.label}</span>
                </button>
              );
            })}
          </div>

          {/* Chapter Body */}
          <div className="flex-1 p-6 overflow-y-auto text-sm text-slate-700 space-y-4 leading-relaxed">
            {activeTab === 'routing' && (
              <div className="space-y-3">
                <h3 className="text-base font-semibold text-slate-900">四象限消息处理机制</h3>
                <div className="p-4 rounded-xl bg-sky-50/90 border border-sky-100">
                  <h4 className="font-semibold text-sky-900 text-xs mb-1">全自动秒回 (Auto-Reply)</h4>
                  <p className="text-xs text-sky-700 leading-relaxed">
                    当命中指定关键词或被 @ 时，由 AI 自动生成回复并直接发送，适合答疑群或高频事务咨询。
                  </p>
                </div>
                <div className="p-4 rounded-xl bg-sky-50/70 border border-sky-100">
                  <h4 className="font-semibold text-sky-900 text-xs mb-1">人机草稿审核 (Copilot Draft)</h4>
                  <p className="text-xs text-sky-700 leading-relaxed">
                    AI 在后台生成拟回复草稿，并放入「草稿箱」等待您确认。您可一键发送、微调编辑或驳回，杜绝任何不当发言。
                  </p>
                </div>
                <div className="p-4 rounded-xl bg-sky-50/50 border border-sky-100/80">
                  <h4 className="font-semibold text-sky-900 text-xs mb-1">纯消息总结 (Summary-Only)</h4>
                  <p className="text-xs text-sky-700 leading-relaxed">
                    AI 保持完全静默不在群内发言，仅持续聚合消息流水，支持随时一键输出结构化纪要与待办清单。
                  </p>
                </div>
                <div className="p-4 rounded-xl bg-sky-50/30 border border-sky-100/60">
                  <h4 className="font-semibold text-sky-900 text-xs mb-1">直通忽略 (Ignore)</h4>
                  <p className="text-xs text-sky-700 leading-relaxed">
                    AI 完全不介入，保持原生聊天状态。
                  </p>
                </div>
              </div>
            )}

            {activeTab === 'opencode' && (
              <div className="space-y-4">
                <h3 className="text-base font-semibold text-slate-900">本地 OpenCode 零配置互联</h3>
                <p>
                  如果您已在当前电脑上安装了 OpenCode，EazyQQ 将自动通过本地端口直连 OpenCode 运行时。
                </p>
                <div className="p-3 rounded-lg bg-sky-50 border border-sky-100 font-mono text-xs text-sky-800">
                  opencode serve --port 4096
                </div>
                <p className="text-xs text-slate-500">
                  无需在客户端内输入任何大模型 API Key，直接复用您在 OpenCode 中已配置好的优质模型与上下文推理能力。
                </p>
              </div>
            )}

            {activeTab === 'files' && (
              <div className="space-y-4">
                <h3 className="text-base font-semibold text-slate-900">群文件同步与知识库</h3>
                <p>
                  所有群文件均可一键自动同步至本地标准工作区目录 <code className="px-1.5 py-0.5 rounded bg-sky-50 border border-sky-100 font-mono text-xs text-sky-800">EazyQQ_Data/</code>。
                </p>
                <ul className="list-disc list-inside space-y-1 text-xs text-slate-600">
                  <li>自动保持文件夹原始层级；</li>
                  <li>原生提取 PDF、DOCX、TXT 文档内容；</li>
                  <li>一键生成文档摘要、核心要点与行动项。</li>
                </ul>
              </div>
            )}

            {activeTab === 'shortcuts' && (
              <div className="space-y-4">
                <h3 className="text-base font-semibold text-slate-900">常用全局与局部快捷键</h3>
                <div className="grid grid-cols-2 gap-3 text-xs">
                  <div className="p-3 rounded-xl border border-slate-200 flex justify-between items-center bg-white">
                    <span>打开/关闭说明书</span>
                    <kbd className="px-1.5 py-0.5 rounded bg-sky-50 border border-sky-100 text-sky-700 font-mono">F1</kbd>
                  </div>
                  <div className="p-3 rounded-xl border border-slate-200 flex justify-between items-center bg-white">
                    <span>发送选中的草稿</span>
                    <kbd className="px-1.5 py-0.5 rounded bg-sky-50 border border-sky-100 text-sky-700 font-mono">Ctrl + Enter</kbd>
                  </div>
                  <div className="p-3 rounded-xl border border-slate-200 flex justify-between items-center bg-white">
                    <span>刷新扫码登录</span>
                    <kbd className="px-1.5 py-0.5 rounded bg-sky-50 border border-sky-100 text-sky-700 font-mono">F5</kbd>
                  </div>
                  <div className="p-3 rounded-xl border border-slate-200 flex justify-between items-center bg-white">
                    <span>快速切换深浅主题</span>
                    <kbd className="px-1.5 py-0.5 rounded bg-sky-50 border border-sky-100 text-sky-700 font-mono">Ctrl + T</kbd>
                  </div>
                </div>
              </div>
            )}

            {activeTab === 'faq' && (
              <div className="space-y-4">
                <h3 className="text-base font-semibold text-slate-900">故障自检与开发者诊断</h3>
                <p>
                  如遇任何使用问题，可在「系统设置」页面点击「一键导出诊断日志包」。系统将自动打包脱敏日志生成 ZIP 文件，可直接发送给开发者查阅定位。
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
