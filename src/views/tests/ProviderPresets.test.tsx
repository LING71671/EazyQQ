import { StrictMode } from 'react';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { SettingsView } from '../SettingsView';
import { AiProviderCard } from '../settings/AiProviderCard';
import type { AppConfig, DependencyHealthReport } from '@/api/contracts';

const mocks = vi.hoisted(() => ({ fetch: vi.fn(), test: vi.fn(), open: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { fetchProviderModels: mocks.fetch, testAiConnection: mocks.test } }));
vi.mock('@tauri-apps/plugin-shell', () => ({ open: mocks.open }));
vi.mock('../settings/models/FreeModelDiscovery', () => ({ FreeModelDiscovery: () => null }));
vi.mock('../settings/WindowBehaviorCard', () => ({ WindowBehaviorCard: () => null }));
vi.mock('../settings/SummaryConfigCard', () => ({ SummaryConfigCard: () => null }));
vi.mock('../settings/DiagnosticsCard', () => ({ DiagnosticsCard: () => null }));
vi.mock('../settings/QqPathConfigCard', () => ({ QqPathConfigCard: () => null }));
vi.mock('../settings/AppUpdateCard', () => ({ AppUpdateCard: () => null }));
vi.mock('../settings/StylePackCard', () => ({ StylePackCard: () => null }));

const config: AppConfig = {
  ai: { activeProvider: 'openai', baseUrl: 'https://relay.example.invalid/v1', model: 'dsv4.1f', apiKey: 'saved-relay-fixture-key', temperature: 0.7, maxContextMessages: 10 },
  summary: { enabled: false, intervalType: '6h', customIntervalMinutes: 360, slidingWindowHours: 6, autoForwardToPhone: false, customPrompt: 'Summary' },
  napcat: { wsPort: 3001, autoRestart: false, heartbeatIntervalSec: 15 },
  storage: { autoSyncFiles: false, maxFileSizeMb: 100 },
  window: { closeToTray: true, minimizeToTray: false },
};
const props = { health: {} as DependencyHealthReport, onCheckHealth: vi.fn(), onExportDiagnostics: vi.fn() };
const modelInput = () => screen.getByLabelText(/模型名称/) as HTMLInputElement;
const keyInput = () => screen.getByLabelText(/API Key（必填）/) as HTMLInputElement;
const urlInput = () => screen.getByLabelText(/接口地址/) as HTMLInputElement;

beforeEach(() => { mocks.fetch.mockReset().mockResolvedValue({ success: true, data: [{ id: 'catalogue-fixture', name: 'catalogue-fixture', isFree: false }] }); mocks.test.mockReset(); mocks.open.mockReset().mockResolvedValue(undefined); });
afterEach(cleanup);

it('offers official and trial choices first while preserving the saved relay and every provider key', async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<SettingsView {...props} config={config} onUpdateConfig={save} />);
  await act(async () => { await Promise.resolve(); });
  expect(modelInput().value).toBe('dsv4.1f');
  expect(urlInput().value).toBe(config.ai.baseUrl);
  expect(keyInput().value).toBe(config.ai.apiKey);
  const choices = screen.getAllByRole('button', { pressed: false }).map(button => button.textContent);
  expect(choices[0]).toContain('DeepSeek 官方');
  expect(choices[1]).toContain('NVIDIA 开发试用');
  expect(choices[2]).toContain('AMD 开发试用');
  expect(choices.at(-1)).toContain('本机 OpenCode');

  fireEvent.click(screen.getByRole('button', { name: /NVIDIA 开发试用/ }));
  expect(urlInput().value).toBe('https://integrate.api.nvidia.com/v1');
  expect(modelInput().value).toBe('deepseek-ai/deepseek-v4-pro');
  expect(keyInput().value).toBe('');
  expect(screen.getByText(/免费开发试用，受额度和限流约束/)).toBeTruthy();
  fireEvent.change(keyInput(), { target: { value: 'nvidia-fixture-key' } });
  fireEvent.click(screen.getByRole('button', { name: /DeepSeek 官方/ }));
  expect(modelInput().value).toBe('deepseek-flash');
  expect(keyInput().value).toBe('');
  fireEvent.change(keyInput(), { target: { value: 'deepseek-fixture-key' } });
  fireEvent.click(screen.getByRole('button', { name: /本机 OpenCode/ }));
  expect((screen.getByLabelText(/OpenCode API Key/) as HTMLInputElement).value).toBe('');
  fireEvent.click(screen.getByRole('button', { name: /OpenAI 兼容端点/ }));
  expect(modelInput().value).toBe('dsv4.1f');
  expect(urlInput().value).toBe(config.ai.baseUrl);
  expect(keyInput().value).toBe(config.ai.apiKey);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存所有设置' })); });
  const saved = save.mock.calls[0][0].ai;
  expect(saved.activeProvider).toBe('openai');
  expect(saved.model).toBe('dsv4.1f');
  expect(saved.providers.nvidia.apiKey).toBe('nvidia-fixture-key');
  expect(saved.providers.deepseek.apiKey).toBe('deepseek-fixture-key');
  expect(saved.providers.opencode.apiKey).toBe('');
});

it('ignores late catalogue replies after switching providers and requires an explicit key before fetching', async () => {
  let finish!: (value: unknown) => void;
  mocks.fetch.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<SettingsView {...props} config={config} onUpdateConfig={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: /AMD 开发试用/ }));
  expect(modelInput().value).toBe('DeepSeek-V4.1-Flash');
  expect(keyInput().value).toBe('');
  expect(mocks.fetch).toHaveBeenCalledTimes(1);
  expect((screen.getByRole('button', { name: /从接口动态获取可用模型/ }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(keyInput(), { target: { value: 'rc-fixture-key' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: /从接口动态获取可用模型/ })); });
  expect(mocks.fetch).toHaveBeenLastCalledWith('amd', 'https://developer.amd.com.cn/radeon/api/v1', 'rc-fixture-key');
  await act(async () => { finish({ success: true, data: [{ id: 'late-relay-model', name: 'late-relay-model' }] }); });
  expect(screen.queryByText('late-relay-model')).toBeNull();
  expect(screen.getAllByText('catalogue-fixture').length).toBeGreaterThan(0);
  expect(modelInput().value).toBe('DeepSeek-V4.1-Flash');
});

it('defaults a missing provider to DeepSeek without issuing a catalogue or inference request under StrictMode', async () => {
  const missing = { ...config, ai: { temperature: 0.7, maxContextMessages: 10 } } as AppConfig;
  render(<StrictMode><SettingsView {...props} config={missing} onUpdateConfig={vi.fn()} /></StrictMode>);
  expect(modelInput().value).toBe('deepseek-flash');
  expect(urlInput().value).toBe('https://api.deepseek.com');
  expect(keyInput().value).toBe('');
  expect(screen.getByRole('button', { name: /DeepSeek 官方/ }).getAttribute('aria-pressed')).toBe('true');
  expect(mocks.fetch).not.toHaveBeenCalled();
  expect(mocks.test).not.toHaveBeenCalled();
});

it('shows a missing-key error without sending a model test and opens only the chosen official entry', async () => {
  render(<AiProviderCard provider="nvidia" baseUrl="https://integrate.api.nvidia.com/v1" model="deepseek-ai/deepseek-v4-pro" apiKey="" onSelectProvider={vi.fn()} onChangeBaseUrl={vi.fn()} onChangeModel={vi.fn()} onChangeApiKey={vi.fn()} fetchedModels={[]} fetchingModels={false} fetchModelError={null} onRefreshModels={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: /测试模型/ }));
  expect(screen.getByText('请先填写当前供应方的 API Key，再测试模型')).toBeTruthy();
  expect(mocks.test).not.toHaveBeenCalled();
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '申请 Key / 官方入口' })); });
  expect(mocks.open).toHaveBeenCalledWith('https://build.nvidia.com/deepseek-ai/deepseek-v4-pro');
});
