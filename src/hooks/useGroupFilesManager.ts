import { useState, useCallback } from 'react';
import { api } from '@/api/client';
import type { GroupFileItemDto, FileSummaryResultDto } from '@/api/contracts';

export function useGroupFilesManager() {
  const [selectedGroupId, setSelectedGroupId] = useState(() => {
    try {
      return localStorage.getItem('eazyqq_selected_group_id') || '';
    } catch {
      return '';
    }
  });

  const [files, setFiles] = useState<GroupFileItemDto[]>([]);
  const [isSyncingFiles, setIsSyncingFiles] = useState(false);
  const [filesError, setFilesError] = useState<string | undefined>(undefined);
  const [activeFileSummary, setActiveFileSummary] = useState<FileSummaryResultDto | undefined>(undefined);
  const [isSummarizingFile, setIsSummarizingFile] = useState(false);

  const handleSyncFiles = useCallback(async () => {
    if (!selectedGroupId) return;
    setIsSyncingFiles(true);
    setFilesError(undefined);
    try {
      const res = await api.getGroupFiles(selectedGroupId);
      if (res.success && res.data) {
        setFiles(res.data);
      } else {
        setFilesError(res.error?.message || '同步群文件失败');
      }
    } catch (e) {
      setFilesError(`同步群文件失败: ${String(e)}`);
      console.error('Failed to sync group files', e);
    } finally {
      setIsSyncingFiles(false);
    }
  }, [selectedGroupId]);

  const handleSelectGroup = useCallback(async (groupId: string) => {
    setSelectedGroupId(groupId);
    try {
      localStorage.setItem('eazyqq_selected_group_id', groupId);
    } catch {}
    setActiveFileSummary(undefined);
    setFilesError(undefined);
    if (!groupId) {
      setFiles([]);
      return;
    }
    // Auto-sync on selection
    setIsSyncingFiles(true);
    try {
      const res = await api.getGroupFiles(groupId);
      if (res.success && res.data) {
        setFiles(res.data);
      } else {
        setFilesError(res.error?.message || '同步群文件失败');
      }
    } catch (e) {
      setFilesError(`同步群文件失败: ${String(e)}`);
    } finally {
      setIsSyncingFiles(false);
    }
  }, []);

  const handleDownloadFile = useCallback(
    async (groupId: string, fileId: string, _fileName: string) => {
      setFiles((prev) =>
        prev.map((f) => (f.fileId === fileId ? { ...f, downloadStatus: 'downloading' } : f))
      );
      try {
        const res = await api.downloadFile(groupId, fileId, _fileName);
        if (res.success && res.data) {
          setFiles((prev) =>
            prev.map((f) =>
              f.fileId === fileId
                ? { ...f, downloadStatus: 'downloaded', localPath: res.data?.localSavePath }
                : f
            )
          );
          setFilesError(undefined);
        } else {
          setFiles((prev) =>
            prev.map((f) => (f.fileId === fileId ? { ...f, downloadStatus: 'remote' } : f))
          );
          setFilesError(res.error?.message || '下载失败');
        }
      } catch (e) {
        setFiles((prev) =>
          prev.map((f) => (f.fileId === fileId ? { ...f, downloadStatus: 'remote' } : f))
        );
        setFilesError(`下载失败: ${String(e)}`);
        console.error('Failed to download file', e);
      }
    },
    []
  );

  const handleSummarizeFile = useCallback(async (localPath: string) => {
    setIsSummarizingFile(true);
    setActiveFileSummary(undefined);
    try {
      const res = await api.summarizeFile(localPath);
      if (res.success && res.data) {
        setActiveFileSummary(res.data);
        setFilesError(undefined);
      } else {
        setFilesError(res.error?.message || '文档综述失败');
      }
    } catch (e) {
      setFilesError(`文档综述失败: ${String(e)}`);
      console.error('Failed to summarize file', e);
    } finally {
      setIsSummarizingFile(false);
    }
  }, []);

  const handleOpenFolder = useCallback(async (targetPath: string) => {
    try {
      await api.openFolder(targetPath);
    } catch (e) {
      console.error('Failed to open folder', e);
    }
  }, []);

  return {
    selectedGroupId,
    setSelectedGroupId,
    files,
    setFiles,
    isSyncingFiles,
    filesError,
    activeFileSummary,
    isSummarizingFile,
    handleSyncFiles,
    handleSelectGroup,
    handleDownloadFile,
    handleSummarizeFile,
    handleOpenFolder,
  };
}
