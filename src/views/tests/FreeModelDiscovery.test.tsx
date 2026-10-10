import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { FreeModelDiscovery } from '../settings/models/FreeModelDiscovery';
import type { FreeModelsReport } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ detect: vi.fn(), listen: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { detectFreeModels: mocks.detect, onFreeModelProgress: mocks.listen } }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const report: FreeModelsReport = {
  runtimeVersion: 'fixture', observedAtMs: 1, catalogueIds: ['plain'], credentialsUsed: false, source: 'fixture',
  models: [{ id: 'plain', name: 'Plain model', state: 'candidate', detail: '待验证', observedAtMs: 1 }],
};

it('loads current candidates without inference and keeps a removed selection unchanged', async () => {
  mocks.detect.mockResolvedValue({ success: true, data: report });
  const choose = vi.fn(); const verified = vi.fn();
  render(<FreeModelDiscovery selectedModel="old-free" onChoose={choose} onVerified={verified} />);
  await act(async () => { await Promise.resolve(); });
  expect(mocks.detect.mock.calls[0][0]).toBe(false);
  expect(screen.getByText(/当前模型 old-free/)).toBeTruthy();
  expect(screen.queryByRole('button', { name: '选用' })).toBeNull();
  expect(choose).not.toHaveBeenCalled();
  expect(verified).toHaveBeenLastCalledWith([]);
});

it('only offers selection after explicit credential-free verification', async () => {
  mocks.detect.mockResolvedValueOnce({ success: true, data: report })
    .mockResolvedValueOnce({ success: true, data: { ...report, models: [{ ...report.models[0], state: 'available', detail: '免凭据已验证' }] } });
  const unsubscribe = vi.fn(); mocks.listen.mockResolvedValue(unsubscribe);
  const choose = vi.fn();
  render(<FreeModelDiscovery selectedModel="old-free" onChoose={choose} onVerified={vi.fn()} />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '自动检测免费模型' })); await Promise.resolve(); });
  expect(mocks.detect.mock.calls[1][0]).toBe(true);
  expect(unsubscribe).toHaveBeenCalled();
  expect(choose).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '选用' }));
  expect(choose).toHaveBeenCalledWith('plain');
});

it('does not claim free access when a result used credentials', async () => {
  mocks.detect.mockResolvedValue({ success: true, data: { ...report, credentialsUsed: true } });
  render(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={vi.fn()} />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByRole('alert').textContent).toContain('不能确认为免凭据');
  expect(screen.queryByRole('button', { name: '选用' })).toBeNull();
});
