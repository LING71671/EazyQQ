import React, { useState } from 'react';
import { FolderSync, Download, FileText, CheckCircle2, Loader2, Sparkles, FolderOpen } from 'lucide-react';
import type { GroupFileItemDto, FileSummaryResultDto } from '@/api/contracts';

interface FilesViewProps {
  files: GroupFileItemDto[];
  onDownloadFile: (groupId: string, fileId: string, fileName: string) => void;
  onSummarizeFile: (localPath: string) => void;
  onOpenFolder: (path: string) => void;
  activeSummary?: FileSummaryResultDto;
  isSummarizing: boolean;
}

export const FilesView: React.FC<FilesViewProps> = ({
  files,
  onDownloadFile,
  onSummarizeFile,
  onOpenFolder,
  activeSummary,
  isSummarizing,
}) => {
  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-slate-200/80 dark:border-slate-800">
        <div>
          <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">群文件全量同步与文档智能</h2>
          <p className="text-xs text-slate-500">一键将群文件下载至本地知识库，并由 AI 提取结构化要点与待办</p>
        </div>
        <button
          onClick={() => onOpenFolder('EazyQQ_Data/files')}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
        >
          <FolderOpen className="w-3.5 h-3.5" />
          <span>打开本地存储目录</span>
        </button>
      </div>

      {/* Main Area (Split: Files list on left, AI Summary on right if active) */}
      <div className="flex-1 flex gap-6 pt-4 overflow-hidden">
        {/* Files Table */}
        <div className="flex-1 overflow-y-auto">
          {files.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-center p-8">
              <img src="/src/assets/empty-files.svg" alt="Empty Files" className="w-36 h-36 mb-4 opacity-70" />
              <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200 mb-1">暂无群文件</h3>
              <p className="text-xs text-slate-400 max-w-sm">
                进入任意已加入的群聊后，群内上传的文档与历史附件将在此处自动建立索引
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {files.map((file) => (
                <div
                  key={file.fileId}
                  className="flex items-center justify-between p-3.5 rounded-xl bg-white dark:bg-slate-900 border border-slate-200/70 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700 transition-colors"
                >
                  {/* File Meta */}
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-9 h-9 rounded-lg bg-sky-50 dark:bg-sky-950/40 text-sky-600 dark:text-sky-400 flex items-center justify-center shrink-0">
                      <FileText className="w-4 h-4" />
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="text-xs font-semibold text-slate-900 dark:text-slate-100 truncate">
                        {file.fileName}
                      </span>
                      <div className="flex items-center gap-2 text-[11px] text-slate-400">
                        <span>{formatSize(file.fileSize)}</span>
                        <span>•</span>
                        <span>上传者: {file.uploaderName}</span>
                      </div>
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 shrink-0">
                    {file.downloadStatus === 'downloaded' ? (
                      <>
                        <span className="flex items-center gap-1 text-[11px] text-emerald-600 dark:text-emerald-400 font-medium px-2 py-0.5 rounded bg-emerald-50 dark:bg-emerald-950/40">
                          <CheckCircle2 className="w-3 h-3" />
                          <span>已同步</span>
                        </span>
                        <button
                          onClick={() => onSummarizeFile(file.localPath || file.fileName)}
                          disabled={isSummarizing}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-purple-50 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300 hover:bg-purple-100 transition-colors"
                        >
                          <Sparkles className="w-3.5 h-3.5" />
                          <span>AI 智能摘要</span>
                        </button>
                      </>
                    ) : file.downloadStatus === 'downloading' ? (
                      <span className="flex items-center gap-1.5 text-xs text-sky-600 animate-pulse px-3 py-1.5">
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        <span>下载同步中...</span>
                      </span>
                    ) : (
                      <button
                        onClick={() => onDownloadFile('default', file.fileId, file.fileName)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                      >
                        <Download className="w-3.5 h-3.5" />
                        <span>下载到本地</span>
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* AI Summary Sidebar Card (if active) */}
        {activeSummary && (
          <div className="w-96 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 flex flex-col shadow-sm overflow-hidden">
            <div className="flex items-center gap-2 pb-3 border-b border-slate-100 dark:border-slate-800 mb-3">
              <Sparkles className="w-4 h-4 text-purple-600 dark:text-purple-400" />
              <h3 className="text-xs font-semibold text-slate-900 dark:text-slate-100 truncate">
                {activeSummary.fileName} 智能摘要
              </h3>
            </div>
            <div className="flex-1 overflow-y-auto space-y-4 text-xs text-slate-700 dark:text-slate-300 leading-relaxed">
              <div>
                <span className="font-semibold text-slate-900 dark:text-slate-100 block mb-1">长文综述：</span>
                <p>{activeSummary.summaryText}</p>
              </div>
              {activeSummary.keyTakeaways.length > 0 && (
                <div>
                  <span className="font-semibold text-slate-900 dark:text-slate-100 block mb-1">核心论点：</span>
                  <ul className="list-disc list-inside space-y-1">
                    {activeSummary.keyTakeaways.map((item, idx) => (
                      <li key={idx}>{item}</li>
                    ))}
                  </ul>
                </div>
              )}
              {activeSummary.actionItems.length > 0 && (
                <div>
                  <span className="font-semibold text-slate-900 dark:text-slate-100 block mb-1">待办事项：</span>
                  <ul className="list-disc list-inside space-y-1">
                    {activeSummary.actionItems.map((item, idx) => (
                      <li key={idx}>{item}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
