import { useEffect, useState } from 'react';
import { api } from '@/api/client';
import type { TelemetryStatus } from '@/api/contracts';
import { testFrontendTelemetry } from '@/features/telemetry/client';

export function TelemetryCard() {
  const [status, setStatus] = useState<TelemetryStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [detail, setDetail] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    api.getTelemetryStatus().then(result => {
      if (!active) return;
      if (!result.success || !result.data || typeof result.data.enabled !== 'boolean' || typeof result.data.configured !== 'boolean') {
        setError(result.error?.message || '诊断状态格式无效'); return;
      }
      setStatus(result.data);
    }).catch(failure => { if (active) setError(`读取诊断状态失败：${failure instanceof Error ? failure.message : String(failure)}`); });
    return () => { active = false; };
  }, []);
  const test = async () => {
    if (busy) return;
    setBusy(true); setError(''); setDetail('');
    try {
      const frontendId = await testFrontendTelemetry();
      const backend = await api.testTelemetry();
      if (!backend.success || !backend.data) throw new Error(backend.error?.message || '后端测试失败');
      if (!backend.data.flushed) throw new Error(`后端发送队列未处理完成，事件编号 ${backend.data.eventId}`);
      setDetail(`前端事件 ${frontendId}；后端事件 ${backend.data.eventId}。发送队列已处理，请在 Sentry 核对接收。`);
    } catch (failure) { setError(failure instanceof Error ? failure.message : String(failure)); }
    finally { setBusy(false); }
  };
  return <section aria-label="Sentry 诊断" className="border-t border-slate-200 pt-4 space-y-3">
    <div className="flex flex-wrap justify-between items-center gap-3">
      <div><h4 className="text-sm font-semibold text-slate-900">Sentry 诊断</h4><p className="text-xs text-slate-600 mt-1">{status?.enabled ? '已启用' : status ? '未启用' : '读取中'}{status?.release ? ` · ${status.release}` : ''}</p></div>
      <button type="button" disabled={!status?.enabled || busy} onClick={() => void test()} className="px-3 py-2 bg-sky-600 text-white rounded-lg text-xs disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600">{busy ? '发送中…' : '发送诊断测试'}</button>
    </div>
    <p className="text-xs text-slate-600">异常、操作耗时和版本关联；不上传会话正文或凭据。</p>
    {detail && <p role="status" className="text-xs text-slate-700 break-all">{detail}</p>}
    {error && <p role="alert" className="text-xs text-red-700 break-words">{error}</p>}
  </section>;
}
