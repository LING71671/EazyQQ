import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { UpdateHistory } from '../settings/updates/UpdateHistory';
import type { UpdateReceipt } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ history: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { getUpdateHistory: mocks.history } }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const receipt: UpdateReceipt = { schemaVersion: 1, operationId: 'a'.repeat(32), component: 'app', targetVersion: '0.7.0', phase: 'installer_succeeded', startedAtMs: 1, updatedAtMs: 2, installerExitCode: 0, error: null };

it('keeps installer success distinct from verified target version', async () => {
  mocks.history.mockResolvedValue({ success: true, data: [receipt] });
  render(<UpdateHistory />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByText('安装程序已结束，待核验版本')).toBeTruthy();
  expect(screen.queryByText('安装完成')).toBeNull();
  expect(screen.getByText('安装程序退出码：0')).toBeTruthy();
});

it('shows a failed installation and allows retrying a failed history read', async () => {
  mocks.history.mockResolvedValueOnce({ success: false, error: { message: '回执损坏' } })
    .mockResolvedValueOnce({ success: true, data: [{ ...receipt, phase: 'failed', installerExitCode: 9, error: '安装程序未成功结束' }] });
  render(<UpdateHistory />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByRole('alert').textContent).toContain('回执损坏');
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '刷新记录' })); await Promise.resolve(); });
  expect(screen.getByText('安装未完成')).toBeTruthy();
  expect(screen.getByText('安装程序退出码：9')).toBeTruthy();
  expect(screen.queryByRole('alert')).toBeNull();
});

it('serializes refresh requests while the first read is pending', async () => {
  let resolve!: (value: unknown) => void;
  mocks.history.mockReturnValue(new Promise(value => { resolve = value; }));
  render(<UpdateHistory />);
  const button = screen.getByRole('button', { name: '读取中…' }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.click(button);
  expect(mocks.history).toHaveBeenCalledTimes(1);
  await act(async () => { resolve({ success: true, data: [] }); });
  expect(screen.getByText('暂无更新记录')).toBeTruthy();
});

it('reports malformed evidence without crashing date rendering', async () => {
  mocks.history.mockResolvedValue({ success: true, data: [{ ...receipt, startedAtMs: Number.MAX_VALUE }] });
  render(<UpdateHistory />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByRole('alert').textContent).toContain('格式无效');
  expect(screen.queryByText('安装程序已结束，待核验版本')).toBeNull();
});

it('shows core deployment and the correlated lifecycle without claiming a running core', async () => {
  mocks.history.mockResolvedValue({ success: true, data: [{ ...receipt, component: 'napcat', operation: 'install', phase: 'deployed', installerExitCode: null,
    events: [{ phase: 'checking', atMs: 1 }, { phase: 'deployed', atMs: 2 }] }] });
  render(<UpdateHistory />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByText('NapCat 核心 v0.7.0')).toBeTruthy();
  expect(screen.getAllByText('核心文件已部署，运行版本待确认').length).toBeGreaterThan(0);
  expect(screen.queryByText('新主程序已启动，版本已核验')).toBeNull();
});

it('rejects verified success without matching observed version evidence', async () => {
  mocks.history.mockResolvedValue({ success: true, data: [{ ...receipt, phase: 'succeeded', verifiedVersion: '0.6.4' }] });
  render(<UpdateHistory />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByRole('alert').textContent).toContain('格式无效');
});
