import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DiagnosticsCard } from '../settings/DiagnosticsCard';
import { ChainHealthDrawer } from '@/components/health/ChainHealthDrawer';
import { api } from '@/api/client';
import type { DependencyHealthReport } from '@/api/contracts';

vi.mock('@/api/client', () => ({ api: { getChainStatus: vi.fn(), repairChain: vi.fn(), restartNapCat: vi.fn() } }));
vi.mock('@/hooks/usePresence', () => ({ usePresence: (open: boolean) => open }));
const props = { health: {} as DependencyHealthReport, onCheckHealth: vi.fn(), onExportDiagnostics: vi.fn() };
function report(health: 'ok' | 'unknown' | 'failed') {
  const link = { link: 'napcat', label: 'NapCat', detail: health === 'unknown' ? '等待扫码' : 'Protocol result', health,
    impact: 'No messages', last_ok_secs_ago: null, last_error_secs_ago: null };
  return { success: true, timestamp: 0, data: { links: [link], firstBreak: health === 'ok' ? null : link, hasFailure: health !== 'ok', uptimeSecs: 1 } };
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getChainStatus).mockResolvedValue(report('ok'));
});
afterEach(cleanup);

it('uses selective recovery and explicitly reports a healthy no-op', async () => {
  vi.mocked(api.repairChain).mockResolvedValue({ success: true, timestamp: 0, data: { action: 'none', attempted: false, ok: true, detail: '健康会话保持连接' } });
  await act(async () => { render(<DiagnosticsCard {...props} />); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '检查并按故障恢复' })); });
  expect(api.repairChain).toHaveBeenCalledTimes(1);
  expect(api.restartNapCat).not.toHaveBeenCalled();
  expect(screen.getByRole('status').textContent).toContain('进程操作：未执行；处理结果：完成。健康会话保持连接');
});

it('does not equate an outer successful response with completed recovery', async () => {
  vi.mocked(api.repairChain).mockResolvedValue({ success: true, timestamp: 0, data: { action: 'start', attempted: true, ok: false, detail: '端口被外部进程占用' } });
  await act(async () => { render(<DiagnosticsCard {...props} />); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '检查并按故障恢复' })); });
  expect(screen.getByRole('alert').textContent).toContain('进程操作：已执行；处理结果：未完成');
  expect(screen.getByRole('alert').textContent).toContain('端口被外部进程占用');
});

it('keeps unknown evidence neutral even when an old firstBreak field is present', async () => {
  vi.mocked(api.getChainStatus).mockResolvedValue(report('unknown'));
  await act(async () => { render(<><DiagnosticsCard {...props} /><ChainHealthDrawer isOpen onClose={vi.fn()} /></>); });
  expect(screen.queryByText(/首要阻断链路/)).toBeNull();
  expect(screen.queryByText(/发现链路断点/)).toBeNull();
  expect(screen.getByText('未发现已确认故障，仍有环节待验证')).toBeTruthy();
  expect(screen.getByText('待验证')).toBeTruthy();
});

it('keeps the drawers recovery evidence visible after the fault clears', async () => {
  vi.mocked(api.getChainStatus).mockResolvedValueOnce(report('failed')).mockResolvedValue(report('ok'));
  vi.mocked(api.repairChain).mockResolvedValue({ success: true, timestamp: 0, data: { action: 'start', attempted: true, ok: true, detail: '缺失服务已启动' } });
  await act(async () => { render(<ChainHealthDrawer isOpen onClose={vi.fn()} />); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '检查并修复' })); });
  expect(screen.getByRole('status').textContent).toContain('进程操作：已执行；处理结果：完成。缺失服务已启动');
  expect(screen.getByText('全链路已验证')).toBeTruthy();
  expect(api.restartNapCat).not.toHaveBeenCalled();
});
