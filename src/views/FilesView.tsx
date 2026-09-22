import React, { useState, useEffect } from 'react';
import {
  FolderSync,
  Download,
  FileText,
  CheckCircle2,
  Loader2,
  Sparkles,
  FolderOpen,
  RefreshCw,
  Eye,
  X,
  Copy,
  Check,
} from 'lucide-react';
import type { ContactItemDto, GroupFileItemDto, FileSummaryResultDto } from '@/api/contracts';
import emptyFilesUrl from '@/assets/empty-files.svg';

interface FilesViewProps {
  files: GroupFileItemDto[];
  /** Groups the account is in - used to pick which group's files to sync. */
  groups: ContactItemDto[];
  selectedGroupId: string;
  onSelectGroup: (groupId: string) => void;
  onSyncFiles: () => void;
  isSyncing: boolean;
  onDownloadFile: (groupId: string, fileId: string, fileName: string) => void;
  onSummarizeFile: (localPath: string) => void;
  onOpenFolder: (path: string) => void;
  activeSummary?: FileSummaryResultDto;
  isSummarizing: boolean;
  errorMessage?: string;
}

export const FilesView: React.FC<FilesViewProps> = ({
  files,
  groups,
  selectedGroupId,
  onSelectGroup,
  onSyncFiles,
  isSyncing,
  onDownloadFile,
  onSummarizeFile,
  onOpenFolder,
  activeSummary,
  isSummarizing,
  errorMessage,
}) => {
  // Local persistence cache for file summaries
  const [summariesCache, setSummariesCache] = useState<Record<string, FileSummaryResultDto>>(() => {
    try {
      const raw = localStorage.getItem('eazyqq_file_summaries_cache');
      return raw ? JSON.parse(raw) : {};
    } catch {
      return {};
    }
  });

  const [viewingSummary, setViewingSummary] = useState<FileSummaryResultDto | null>(activeSummary || null);
  const [copiedSummary, setCopiedSummary] = useState(false);
  const [summarizingFileId, setSummarizingFileId] = useState<string | null>(null);

  useEffect(() => {
    if (activeSummary) {
      setViewingSummary(activeSummary);
      setSummariesCache((prev) => {
        const next = { ...prev, [activeSummary.fileName]: activeSummary };
        try {
          localStorage.setItem('eazyqq_file_summaries_cache', JSON.stringify(next));
        } catch {}
        return next;
      });
      setSummarizingFileId(null);
    }
  }, [activeSummary]);

  const handleCopySummary = async () => {
    if (!viewingSummary) return;
    const text = [
      `【${viewingSummary.fileName} 智能摘要】`,
      '',
      '■ 全文综述：',
      viewingSummary.summaryText,
      '',
      viewingSummary.keyTakeaways?.length
        ? '■ 核心要点：\n' + viewingSummary.keyTakeaways.map((t, i) => `${i + 1}. ${t}`).join('\n')
        : '',
      '',
      viewingSummary.actionItems?.length
        ? '■ 待办行动：\n' + viewingSummary.actionItems.map((a, i) => `[ ] ${a}`).join('\n')
        : '',
    ]
      .filter(Boolean)
      .join('\n');

    try {
      await navigator.clipboard.writeText(text);
      setCopiedSummary(true);
      setTimeout(() => setCopiedSummary(false), 2000);
    } catch (e) {
      console.error('Failed to copy summary', e);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const dedupedFiles = React.useMemo(() => {
    const map = new Map<string, GroupFileItemDto>();
    for (const file of files) {
      const key = file.fileName.trim();
      const existing = map.get(key);
      if (!existing) {
        map.set(key, file);
      } else {
        const isDownloaded = file.downloadStatus === 'downloaded' || existing.downloadStatus === 'downloaded';
        const isDownloading = file.downloadStatus === 'downloading' || existing.downloadStatus === 'downloading';
        const localPath = file.localPath || existing.localPath;
        const isNum = (s: string) => /^\d+$/.test(s.trim());
        const uploaderName = !isNum(file.uploaderName) && file.uploaderName.trim().length > 0
          ? file.uploaderName
          : existing.uploaderName;

        map.set(key, {
          ...existing,
          ...file,
          fileId: existing.downloadStatus === 'downloaded' ? existing.fileId : file.fileId,
          downloadStatus: isDownloaded ? 'downloaded' : (isDownloading ? 'downloading' : 'remote'),
          localPath,
          uploaderName,
        });
      }
    }
    return Array.from(map.values());
  }, [files]);

  return (
    <div className="flex-1 h-full p-6 flex flex-col select-none overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-slate-200/80 dark:border-slate-800">
        <div>
          <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">群文件全量同步与文档智能</h2>
          <p className="text-xs text-slate-500">一键将群文件下载至本地知识库，并由 AI 提取结构化要点与待办</p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={selectedGroupId}
            onChange={(e) => onSelectGroup(e.target.value)}
            className="max-w-[220px] px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-xs text-slate-700 dark:text-slate-200 focus:outline-none focus:border-sky-500 cursor-pointer"
          >
            <option value="">选择要同步的群聊…</option>
            {groups.map((g) => (
              <option key={g.targetId} value={g.targetId}>
                {g.name}
              </option>
            ))}
          </select>
          <button
            onClick={onSyncFiles}
            disabled={!selectedGroupId || isSyncing}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-50 disabled:cursor-not-allowed transition-all cursor-pointer active:scale-95"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
            <span>{isSyncing ? '同步中…' : '同步群文件'}</span>
          </button>
          <button
            onClick={() => onOpenFolder(selectedGroupId ? `group_files/${selectedGroupId}` : 'group_files')}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-all cursor-pointer active:scale-95"
          >
            <FolderOpen className="w-3.5 h-3.5" />
            <span>打开本地存储目录</span>
          </button>
        </div>
      </div>

      {errorMessage && (
        <div className="mt-3 p-3 rounded-xl bg-amber-50 border border-amber-200 text-[11px] text-amber-900 leading-relaxed">
          {errorMessage}
        </div>
      )}

      {/* Main Area (Split: Files list on left, AI Summary on right if active) */}
      <div className="flex-1 flex gap-6 pt-4 overflow-hidden">
        {/* Files Table */}
        <div className="flex-1 overflow-y-auto">
          {dedupedFiles.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-center p-8">
              <img src={emptyFilesUrl} alt="Empty Files" className="w-36 h-36 mb-4 opacity-70" />
              <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200 mb-1">暂无群文件</h3>
              <p className="text-xs text-slate-400 max-w-sm">
                进入任意已加入的群聊后，群内上传的文档与历史附件将在此处自动建立索引
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {dedupedFiles.map((file) => (
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
                        {(() => {
                          const fileSum =
                            summariesCache[file.fileId] ||
                            summariesCache[file.fileName] ||
                            (activeSummary?.fileName === file.fileName ? activeSummary : undefined);
                          const isThisSummarizing = isSummarizing && summarizingFileId === file.fileId;

                          if (fileSum) {
                            return (
                              <div className="flex items-center gap-1.5">
                                <button
                                  onClick={() => {
                                    setViewingSummary(fileSum);
                                    setCopiedSummary(false);
                                  }}
                                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-purple-600 hover:bg-purple-700 active:bg-purple-800 text-white shadow-xs transition-all cursor-pointer active:scale-95"
                                >
                                  <Eye className="w-3.5 h-3.5" />
                                  <span>查看摘要</span>
                                </button>
                                <button
                                  onClick={() => {
                                    setSummarizingFileId(file.fileId);
                                    onSummarizeFile(file.localPath || file.fileName);
                                  }}
                                  disabled={isSummarizing}
                                  title="重新生成 AI 摘要"
                                  className="p-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-slate-500 hover:text-purple-600 hover:border-purple-200 hover:bg-purple-50 transition-all cursor-pointer active:scale-95 disabled:opacity-50"
                                >
                                  <RefreshCw className={`w-3.5 h-3.5 ${isThisSummarizing ? 'animate-spin text-purple-600' : ''}`} />
                                </button>
                              </div>
                            );
                          }

                          if (isThisSummarizing) {
                            return (
                              <span className="flex items-center gap-1.5 text-xs text-purple-600 dark:text-purple-400 font-medium animate-pulse px-3 py-1.5 bg-purple-50 dark:bg-purple-950/40 rounded-lg">
                                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                <span>AI 提炼中…</span>
                              </span>
                            );
                          }

                          return (
                            <button
                              onClick={() => {
                                setSummarizingFileId(file.fileId);
                                onSummarizeFile(file.localPath || file.fileName);
                              }}
                              disabled={isSummarizing}
                              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-purple-50 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300 hover:bg-purple-100 dark:hover:bg-purple-900/50 transition-all cursor-pointer active:scale-95 disabled:opacity-50"
                            >
                              <Sparkles className="w-3.5 h-3.5" />
                              <span>AI 智能摘要</span>
                            </button>
                          );
                        })()}
                      </>
                    ) : file.downloadStatus === 'downloading' ? (
                      <span className="flex items-center gap-1.5 text-xs text-sky-600 animate-pulse px-3 py-1.5">
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        <span>下载同步中...</span>
                      </span>
                    ) : (
                      <button
                        onClick={() => onDownloadFile(selectedGroupId, file.fileId, file.fileName)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-700 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-all cursor-pointer active:scale-95"
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

        {/* AI Summary Sidebar Drawer (if active) */}
        {viewingSummary && (
          <div className="w-[420px] max-w-full bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl p-5 flex flex-col shadow-lg overflow-hidden shrink-0">
            {/* Header */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100 dark:border-slate-800 mb-3">
              <div className="flex items-center gap-2 min-w-0 pr-2">
                <div className="w-8 h-8 rounded-lg bg-purple-50 dark:bg-purple-950/50 text-purple-600 dark:text-purple-400 flex items-center justify-center shrink-0">
                  <Sparkles className="w-4 h-4" />
                </div>
                <div className="min-w-0">
                  <h3 className="text-xs font-semibold text-slate-900 dark:text-slate-100 truncate">
                    {viewingSummary.fileName}
                  </h3>
                  <span className="text-[11px] text-slate-400">
                    文档摘要 · 提炼自 {viewingSummary.totalChars || '—'} 字文本
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-1 shrink-0">
                <button
                  onClick={handleCopySummary}
                  className="flex items-center gap-1 px-2.5 py-1 rounded-lg border border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 text-[11px] text-slate-600 dark:text-slate-300 transition-colors cursor-pointer"
                  title="复制完整摘要"
                >
                  {copiedSummary ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                      <span className="text-emerald-600 dark:text-emerald-400 font-medium">已复制</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      <span>复制</span>
                    </>
                  )}
                </button>
                <button
                  onClick={() => setViewingSummary(null)}
                  className="p-1 rounded-lg text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                  title="关闭摘要面板"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            </div>

            {/* Body */}
            <div className="flex-1 overflow-y-auto space-y-4 text-xs text-slate-700 dark:text-slate-300 leading-relaxed pr-1">
              <div className="p-3.5 rounded-xl bg-purple-50/50 dark:bg-purple-950/20 border border-purple-100/80 dark:border-purple-900/30">
                <span className="font-semibold text-purple-900 dark:text-purple-300 block mb-1.5 flex items-center gap-1.5">
                  <span>📄 全文综合要点</span>
                </span>
                <p className="text-slate-800 dark:text-slate-200 whitespace-pre-wrap leading-relaxed text-[12px]">
                  {viewingSummary.summaryText}
                </p>
              </div>

              {viewingSummary.keyTakeaways && viewingSummary.keyTakeaways.length > 0 && (
                <div className="space-y-2">
                  <span className="font-semibold text-slate-900 dark:text-slate-100 block text-xs">
                    💡 核心论点与结论 ({viewingSummary.keyTakeaways.length})
                  </span>
                  <div className="space-y-1.5">
                    {viewingSummary.keyTakeaways.map((item, idx) => (
                      <div
                        key={idx}
                        className="p-2.5 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-100 dark:border-slate-800 flex items-start gap-2"
                      >
                        <span className="w-4 h-4 rounded-full bg-sky-100 text-sky-700 dark:bg-sky-950 dark:text-sky-300 text-[10px] font-bold flex items-center justify-center shrink-0 mt-0.5">
                          {idx + 1}
                        </span>
                        <span className="text-slate-700 dark:text-slate-200 text-[11px] leading-relaxed">
                          {item}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {viewingSummary.actionItems && viewingSummary.actionItems.length > 0 && (
                <div className="space-y-2">
                  <span className="font-semibold text-slate-900 dark:text-slate-100 block text-xs">
                    📌 待办事项与行动项 ({viewingSummary.actionItems.length})
                  </span>
                  <div className="space-y-1.5">
                    {viewingSummary.actionItems.map((item, idx) => (
                      <div
                        key={idx}
                        className="p-2.5 rounded-lg bg-emerald-50/50 dark:bg-emerald-950/20 border border-emerald-100 dark:border-emerald-900/30 flex items-start gap-2"
                      >
                        <CheckCircle2 className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5" />
                        <span className="text-emerald-900 dark:text-emerald-200 text-[11px] leading-relaxed">
                          {item}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="pt-3 mt-3 border-t border-slate-100 dark:border-slate-800 flex items-center justify-between text-[11px]">
              <button
                onClick={() => onOpenFolder('EazyQQ_Data/group_files')}
                className="flex items-center gap-1 text-slate-500 hover:text-sky-600 dark:hover:text-sky-400 transition-colors cursor-pointer"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>打开本地存储目录</span>
              </button>
              <button
                onClick={() => {
                  const target = dedupedFiles.find((f) => f.fileName === viewingSummary.fileName);
                  if (target) {
                    setSummarizingFileId(target.fileId);
                    onSummarizeFile(target.localPath || target.fileName);
                  }
                }}
                disabled={isSummarizing}
                className="flex items-center gap-1 text-purple-600 hover:text-purple-700 dark:text-purple-400 transition-colors cursor-pointer font-medium disabled:opacity-50"
              >
                <RefreshCw className={`w-3.5 h-3.5 ${isSummarizing ? 'animate-spin' : ''}`} />
                <span>重新生成摘要</span>
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
