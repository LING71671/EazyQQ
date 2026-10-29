import { RefreshCw } from 'lucide-react';
import type { UpdateReceipt } from '@/api/contracts';
import { useUpdateHistory } from '@/features/updates/useUpdateHistory';

const labels: Record<UpdateReceipt['phase'], string> = {
  checking: '正在检查官方发布',
  checked: '版本检查已完成',
  downloading: '正在下载更新包',
  ready: '更新包已校验',
  handoff: '已交接，待助手确认',
  waiting_parent: '等待调用程序退出',
  verifying: '正在校验更新包',
  installing: '安装程序运行中',
  installer_succeeded: '安装程序已结束，待核验版本',
  pending_verification: '安装程序已结束，待核验版本',
  deployed: '核心文件已部署，运行版本待确认',
  succeeded: '新主程序已启动，版本已核验',
  failed: '安装未完成',
  aborted: '更新任务已中止',
};
function phaseLabel(receipt: UpdateReceipt, phase: UpdateReceipt['phase']) {
  if (phase === 'failed' && receipt.operation === 'check') return '更新检查失败';
  if (phase === 'failed' && receipt.component === 'napcat') return '核心更新未完成';
  return labels[phase];
}

export function UpdateHistory() {
  const history = useUpdateHistory();
  return <section aria-labelledby="update-history-heading" aria-busy={history.loading} className="border-t border-slate-200 pt-4 space-y-3">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <h4 id="update-history-heading" className="text-sm font-semibold text-slate-900">更新记录</h4>
      <button type="button" onClick={() => void history.refresh()} disabled={history.loading}
        className="inline-flex items-center gap-1.5 px-3 py-2 text-xs text-sky-700 hover:bg-sky-50 rounded-lg disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600">
        <RefreshCw aria-hidden="true" className={`w-3.5 h-3.5 ${history.loading ? 'animate-spin' : ''}`} />
        {history.loading ? '读取中…' : '刷新记录'}
      </button>
    </div>
    {history.error && <p role="alert" className="text-xs text-red-700 break-words">记录未刷新：{history.error}</p>}
    {!history.loading && !history.error && history.receipts.length === 0 && <p className="text-xs text-slate-600">暂无更新记录</p>}
    <ol className="divide-y divide-slate-200 max-h-80 overflow-y-auto" aria-label="程序与核心更新记录">
      {history.receipts.map(receipt => <li key={receipt.operationId} data-update-operation={receipt.operationId} className="py-3 space-y-1.5">
        <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
          <p className="text-sm font-medium text-slate-900">{receipt.component === 'napcat' ? 'NapCat 核心' : '主程序'} {receipt.targetVersion ? `v${receipt.targetVersion}` : '版本未记录'}{receipt.operation === 'check' ? ' · 检查' : ''}</p>
          <time className="text-xs text-slate-600 tabular-nums" dateTime={new Date(receipt.startedAtMs).toISOString()}>{new Date(receipt.startedAtMs).toLocaleString('zh-CN')}</time>
        </div>
        <p className={`text-xs ${receipt.phase === 'failed' ? 'text-red-700' : 'text-slate-700'}`}>{phaseLabel(receipt, receipt.phase)}</p>
        {['checking', 'downloading', 'verifying', 'ready', 'handoff', 'waiting_parent', 'installing'].includes(receipt.phase) && <p className="text-xs text-slate-600">任务未确认结束；以上为最后记录阶段，刷新可查询最终结果。</p>}
        {receipt.verifiedVersion && <p className="text-xs text-slate-600">已核验主程序版本：{receipt.verifiedVersion}</p>}
        {receipt.installerExitCode !== null && <p className="text-xs text-slate-600">安装程序退出码：{receipt.installerExitCode}</p>}
        {receipt.error && <p className="text-xs text-red-700 break-words">{receipt.error}</p>}
        <details className="text-xs text-slate-600">
          <summary className="cursor-pointer py-1 focus-visible:outline-2 focus-visible:outline-sky-600">更新编号</summary>
          <code className="block break-all font-mono">{receipt.operationId}</code>
          {receipt.events && receipt.events.length > 0 && <ol className="mt-2 space-y-1" aria-label="更新阶段记录">{receipt.events.map((event, index) => <li key={`${event.atMs}-${index}`} className="flex flex-wrap justify-between gap-x-4 gap-y-1"><span>{phaseLabel(receipt, event.phase)}</span><time className="tabular-nums" dateTime={new Date(event.atMs).toISOString()}>{new Date(event.atMs).toLocaleString('zh-CN')}</time></li>)}</ol>}
        </details>
      </li>)}
    </ol>
  </section>;
}
