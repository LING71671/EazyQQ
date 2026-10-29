import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { StrictMode } from 'react';
import { FreeModelDiscovery } from '../settings/models/FreeModelDiscovery';
import type { FreeModelsReport } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ detect: vi.fn(), listen: vi.fn(), cancel: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { detectFreeModels: mocks.detect, onFreeModelProgress: mocks.listen, cancelFreeModelDetection: mocks.cancel } }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const report: FreeModelsReport = {
  runtimeVersion: 'fixture', observedAtMs: 1, catalogueIds: ['plain'], credentialsUsed: false, source: 'fixture',
  requestId: 'fixture', status: 'completed', completed: 1, total: 1, catalogueRefreshed: true, error: null, cleanupError: null, history: [],
  models: [{ id: 'plain', name: 'Plain model', state: 'candidate', detail: '待验证', observedAtMs: 1 }],
};

it('blocks a probe in the first render before the catalogue effect starts', () => {
  const initial = document.createElement('div');
  initial.innerHTML = renderToStaticMarkup(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={vi.fn()} />);
  expect(initial.querySelector('button')?.disabled).toBe(true);
  expect(initial.querySelector('section')?.getAttribute('aria-busy')).toBe('true');
  expect(mocks.detect).not.toHaveBeenCalled();
});

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

it('stops the exact active probe and preserves its completed model evidence', async () => {
  let complete!: (value: unknown) => void;
  mocks.detect.mockResolvedValueOnce({ success: true, data: report }).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  mocks.listen.mockResolvedValue(vi.fn());
  mocks.cancel.mockResolvedValue({ success: true, data: true });
  const verified = vi.fn();
  render(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={verified} />);
  await act(async () => {});
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '自动检测免费模型' })); });
  const requestId = mocks.detect.mock.calls[1][1];
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '停止检测' })); });
  expect(mocks.cancel).toHaveBeenCalledWith(requestId);
  expect(screen.getByRole('button', { name: '正在停止…' }).getAttribute('disabled')).not.toBeNull();
  await act(async () => { complete({ success: true, data: { ...report, status: 'cancelled', total: 3, models: [{ ...report.models[0], state: 'available' }] } }); });
  expect(screen.getByText(/检测已停止 · 已完成 1\/3/)).toBeTruthy();
  expect(verified).toHaveBeenLastCalledWith(['plain']);
  expect(screen.getByRole('button', { name: '自动检测免费模型' }).getAttribute('disabled')).toBeNull();
});

it('cancels and unsubscribes on unmount and ignores a late result', async () => {
  let complete!: (value: unknown) => void;
  mocks.detect.mockResolvedValueOnce({ success: true, data: report }).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  const unlisten = vi.fn(); mocks.listen.mockResolvedValue(unlisten);
  mocks.cancel.mockResolvedValue({ success: true, data: true });
  const verified = vi.fn();
  const view = render(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={verified} />);
  await act(async () => {});
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '自动检测免费模型' })); });
  const requestId = mocks.detect.mock.calls[1][1];
  view.unmount();
  expect(mocks.cancel).toHaveBeenCalledWith(requestId);
  expect(unlisten).toHaveBeenCalledTimes(1);
  const calls = verified.mock.calls.length;
  await act(async () => { complete({ success: true, data: { ...report, models: [{ ...report.models[0], state: 'available' }] } }); });
  expect(verified).toHaveBeenCalledTimes(calls);
  expect(unlisten).toHaveBeenCalledTimes(1);
});

it('disposes a late listener without starting a probe after unmount', async () => {
  let subscribed!: (unlisten: () => void) => void;
  mocks.detect.mockResolvedValue({ success: true, data: report });
  mocks.listen.mockImplementation(() => new Promise(resolve => { subscribed = resolve; }));
  const view = render(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={vi.fn()} />);
  await act(async () => {});
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '自动检测免费模型' })); });
  view.unmount();
  const unlisten = vi.fn();
  await act(async () => { subscribed(unlisten); });
  expect(unlisten).toHaveBeenCalledTimes(1);
  expect(mocks.detect).toHaveBeenCalledTimes(1);
});

it('can stop before the listener is ready without starting model inference', async () => {
  let subscribed!: (unlisten: () => void) => void;
  mocks.detect.mockResolvedValue({ success: true, data: report });
  mocks.listen.mockImplementation(() => new Promise(resolve => { subscribed = resolve; }));
  render(<FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={vi.fn()} />);
  await act(async () => {});
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '自动检测免费模型' })); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '停止检测' })); subscribed(vi.fn()); });
  expect(mocks.detect).toHaveBeenCalledTimes(1);
  expect(screen.getByText(/检测已停止 · 已完成 0\/0/)).toBeTruthy();
});

it('keeps historical free access separate from current qualification after failure', async () => {
  mocks.detect.mockResolvedValue({ success: true, data: { ...report, status: 'failed', models: [], catalogueRefreshed: false, error: '目录刷新失败',
    history: [{ requestId: 'past', status: 'completed', observedAtMs: 1, models: [{ ...report.models[0], state: 'available' }] }] } });
  const verified = vi.fn();
  render(<FreeModelDiscovery selectedModel="not-refreshed" onChoose={vi.fn()} onVerified={verified} />);
  await act(async () => {});
  expect(screen.getByRole('alert').textContent).toContain('目录刷新失败');
  expect(screen.getByText(/历史检测证据/)).toBeTruthy();
  expect(screen.getByText(/历史：免凭据可用/)).toBeTruthy();
  expect(screen.queryByText(/当前模型 not-refreshed/)).toBeNull();
  expect(screen.queryByRole('button', { name: '选用' })).toBeNull();
  expect(verified).toHaveBeenLastCalledWith([]);
});

it('serializes StrictMode setup and cleanup without overlapping catalogue requests', async () => {
  mocks.detect.mockResolvedValue({ success: true, data: report });
  mocks.cancel.mockResolvedValue({ success: true, data: true });
  render(<StrictMode><FreeModelDiscovery selectedModel="plain" onChoose={vi.fn()} onVerified={vi.fn()} /></StrictMode>);
  await act(async () => {});
  expect(mocks.detect).toHaveBeenCalledTimes(1);
  expect(mocks.detect.mock.calls[0][0]).toBe(false);
  expect(screen.getByRole('button', { name: '自动检测免费模型' }).getAttribute('disabled')).toBeNull();
});
