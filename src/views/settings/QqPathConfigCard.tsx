import React, { useState, useEffect } from 'react';
import { FolderCheck, Save, CheckCircle2, AlertCircle, RefreshCw } from 'lucide-react';
import { api } from '@/api/client';

export const QqPathConfigCard: React.FC = () => {
  const [qqPath, setQqPath] = useState('');
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fetchCurrentPath = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.getQqPath();
      if (res.success && res.data) {
        setQqPath(res.data);
      } else {
        setError(res.error?.message || '未能自动探测到本地 QQ 安装路径');
      }
    } catch (e: any) {
      setError(e?.message || String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchCurrentPath();
  }, []);

  const handleSavePath = async () => {
    if (!qqPath.trim()) {
      setError('请输入有效的 QQ.exe 完整路径');
      return;
    }
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      const res = await api.setQqPath(qqPath.trim());
      if (res.success) {
        setNotice('QQ 安装路径已成功保存并同步配置！');
        setTimeout(() => setNotice(null), 3000);
      } else {
        setError(res.error?.message || '路径无效或文件不存在');
      }
    } catch (e: any) {
      setError(e?.message || String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="p-5 rounded-2xl bg-white border border-slate-200/80 shadow-xs space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-5 h-5 rounded-md bg-sky-50 text-sky-600 flex items-center justify-center">
            <FolderCheck className="w-3.5 h-3.5" />
          </div>
          <div>
            <h4 className="text-xs font-semibold text-slate-900">QQNT 客户端安装路径</h4>
            <p className="text-[11px] text-slate-400 mt-0.5">
              自定义本地电脑 QQ.exe 路径（若已移动安装目录或使用非标准盘符，请在此指定）
            </p>
          </div>
        </div>
        <button
          onClick={fetchCurrentPath}
          disabled={loading}
          title="重新自动嗅探路径"
          className="p-1.5 rounded-lg text-slate-400 hover:text-slate-700 hover:bg-slate-100 transition-colors disabled:opacity-50 cursor-pointer"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <input
          type="text"
          value={qqPath}
          onChange={(e) => setQqPath(e.target.value)}
          placeholder="例如：C:\Program Files\Tencent\QQNT\QQ.exe"
          className="flex-1 px-3 py-2 rounded-xl border border-slate-200 text-xs text-slate-800 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-sky-500/20 focus:border-sky-500 font-mono transition-all"
        />
        <button
          onClick={handleSavePath}
          disabled={saving}
          className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-slate-900 hover:bg-slate-800 text-white text-xs font-semibold transition-all shrink-0 cursor-pointer active:scale-95 disabled:opacity-50"
        >
          <Save className="w-3.5 h-3.5" />
          <span>{saving ? '保存中...' : '保存路径'}</span>
        </button>
      </div>

      {notice && (
        <div className="p-2.5 rounded-xl bg-emerald-50 border border-emerald-100 text-[11px] text-emerald-700 flex items-center gap-2">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          <span>{notice}</span>
        </div>
      )}

      {error && (
        <div className="p-2.5 rounded-xl bg-red-50 border border-red-100 text-[11px] text-red-600 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
};
