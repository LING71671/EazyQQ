import type { RepairReport } from '@/api/contracts';

export function describeRepair(report: RepairReport): string {
  const attempted = report.attempted ? '已执行' : '未执行';
  const outcome = report.ok ? '完成' : '未完成';
  return `进程操作：${attempted}；处理结果：${outcome}。${report.detail}`;
}
