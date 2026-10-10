import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { AiProviderCard } from '../settings/AiProviderCard';
import { DiagnosticsCard } from '../settings/DiagnosticsCard';
import { notifyAiTestFinished } from '@/features/health/useChainStatus';

const mocks = vi.hoisted(() => ({ chain: vi.fn(), test: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { getChainStatus: mocks.chain, testAiConnection: mocks.test } }));
vi.mock('../settings/models/FreeModelDiscovery', () => ({ FreeModelDiscovery: () => null }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const report = (health: 'ok' | 'unknown') => ({ success: true, data: { links: [{ link: 'ai_provider', label: '大模型', health, impact: '', detail: health === 'ok' ? '模型已通过推理验证' : '模型已配置，尚未验证推理能力', last_ok_secs_ago: null, last_error_secs_ago: null }], firstBreak: null, hasFailure: false, uptimeSecs: 1 } });
const diagnostics = { health: { isAllReady: true, qqNt: {ready:true,path:''}, openCode: {ready:true}, ports: {napcatPort:0,opencodePort:0,isConflict:false}, storage: {workspacePath:'',isWritable:true,freeSpaceMb:1} }, onCheckHealth: vi.fn(), onExportDiagnostics: vi.fn() };
const provider = { provider: 'opencode' as const, model: 'a', baseUrl: 'https://opencode.ai/zen/v1', apiKey: '', fetchedModels: [], fetchingModels: false, fetchModelError: null, onSelectProvider: vi.fn(), onChangeModel: vi.fn(), onChangeBaseUrl: vi.fn(), onChangeApiKey: vi.fn(), onRefreshModels: vi.fn() };

it('refreshes diagnostics after a successful explicit test and offers no free-channel switch', async () => {
  let healthy = false;
  mocks.chain.mockImplementation(async () => report(healthy ? 'ok' : 'unknown'));
  mocks.test.mockImplementation(async () => { healthy = true; return { success: true, data: { isSuccess: true, reply: 'OK', matchesActiveConfig: true } }; });
  const { container } = render(<><AiProviderCard {...provider} /><DiagnosticsCard {...diagnostics} /></>);
  await act(async () => { await Promise.resolve(); });
  expect(container.querySelector('[data-chain-link="ai_provider"]')?.getAttribute('data-health')).toBe('unknown');
  expect(screen.queryByText(/切.*免费通道/)).toBeNull();
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: /测试模型/ })); });
  expect(container.querySelector('[data-chain-link="ai_provider"]')?.getAttribute('data-health')).toBe('ok');
  expect(screen.getByText('当前生效配置已通过验证，诊断状态已同步。')).toBeTruthy();
  expect(mocks.test.mock.calls[0][0].apiKey).toBe('');
});

it('rejects an old pending snapshot when test completion requests a newer one', async () => {
  let resolve!: (value: ReturnType<typeof report>) => void;
  mocks.chain.mockImplementationOnce(() => new Promise(done => { resolve = done; })).mockResolvedValue(report('ok'));
  const { container } = render(<DiagnosticsCard {...diagnostics} />);
  await act(async () => { notifyAiTestFinished(); resolve(report('unknown')); });
  expect(container.querySelector('[data-chain-link="ai_provider"]')?.getAttribute('data-health')).toBe('ok');
});

it('discards a test reply after the edited model changes', async () => {
  let resolve!: (value: unknown) => void;
  mocks.test.mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  const view = render(<AiProviderCard {...provider} />);
  fireEvent.click(screen.getByRole('button', { name: /测试模型/ }));
  view.rerender(<AiProviderCard {...provider} model="b" isDraft />);
  await act(async () => { resolve({ success: true, data: { reply: 'OLD REPLY' } }); });
  expect(screen.queryByText('OLD REPLY')).toBeNull();
});
