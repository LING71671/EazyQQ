import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useState } from 'react';
import { SettingsView } from '../SettingsView';
import type { AppConfig, DependencyHealthReport } from '@/api/contracts';

vi.mock('@/api/client', () => ({ api: { fetchProviderModels: vi.fn().mockResolvedValue({ success: true, data: [{ id: 'saved-model' }] }) } }));
vi.mock('../settings/AiProviderCard', () => ({
  AI_PRESETS: { opencode: { baseUrl: 'https://example.test' } },
  AiProviderCard: ({ model, onChangeModel }: { model: string; onChangeModel: (value: string) => void }) =>
    <input aria-label="模型" value={model} onChange={event => onChangeModel(event.target.value)} />,
}));
vi.mock('../settings/WindowBehaviorCard', () => ({ WindowBehaviorCard: ({ closeToTray, onChangeBehavior }: {
  closeToTray: boolean; onChangeBehavior: (value: { closeToTray: boolean }) => void;
}) => <button onClick={() => onChangeBehavior({ closeToTray: !closeToTray })}>切换关闭行为</button> }));
vi.mock('../settings/SummaryConfigCard', () => ({ SummaryConfigCard: () => null }));
vi.mock('../settings/DiagnosticsCard', () => ({ DiagnosticsCard: () => null }));
vi.mock('../settings/QqPathConfigCard', () => ({ QqPathConfigCard: () => null }));
vi.mock('../settings/AppUpdateCard', () => ({ AppUpdateCard: () => null }));

const config = { ai: { activeProvider: 'opencode', model: 'saved-model', baseUrl: 'https://example.test', temperature: 0.7, maxContextMessages: 10 },
  summary: { enabled: false, intervalType: '6h', customIntervalMinutes: 360, slidingWindowHours: 6, autoForwardToPhone: false, customPrompt: 'Summary' },
  napcat: { wsPort: 3001, autoRestart: false, heartbeatIntervalSec: 15 }, storage: { autoSyncFiles: false, maxFileSizeMb: 100 },
  window: { closeToTray: true, minimizeToTray: false } } satisfies AppConfig;
const props = { config, health: {} as DependencyHealthReport, onCheckHealth: vi.fn(), onExportDiagnostics: vi.fn() };
beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

it('waits for persistence, blocks repeat saves, and only then announces success', async () => {
  let finish!: () => void;
  const update = vi.fn((_patch: Partial<AppConfig>) => new Promise<void>(resolve => { finish = resolve; }));
  render(<SettingsView {...props} onUpdateConfig={update} />);
  fireEvent.change(screen.getByLabelText('模型'), { target: { value: 'edited-model' } });
  fireEvent.click(screen.getByRole('button', { name: '保存所有设置' }));
  expect(screen.queryByText('已保存')).toBeNull();
  expect((screen.getByRole('button', { name: '正在保存…' }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole('button', { name: '正在保存…' }));
  expect(update).toHaveBeenCalledTimes(1);
  await act(async () => { finish(); });
  expect(screen.getByRole('button', { name: '已保存' })).toBeTruthy();
  expect(update.mock.calls[0][0].ai?.model).toBe('edited-model');
});

it('shows persistence failure and retains edited settings for retry', async () => {
  const update = vi.fn().mockRejectedValueOnce(new Error('Disk is read-only')).mockResolvedValue(undefined);
  render(<SettingsView {...props} onUpdateConfig={update} />);
  fireEvent.change(screen.getByLabelText('模型'), { target: { value: 'keep-this-draft' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存所有设置' })); });
  expect(screen.getByRole('alert').textContent).toContain('Disk is read-only');
  expect((screen.getByLabelText('模型') as HTMLInputElement).value).toBe('keep-this-draft');
  expect(screen.queryByText('已保存')).toBeNull();
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存所有设置' })); });
  expect(screen.getByRole('button', { name: '已保存' })).toBeTruthy();
});

it('preserves an unsaved model while persisting only window behavior', async () => {
  function Host() {
    const [saved, setSaved] = useState<AppConfig>(config);
    return <SettingsView {...props} config={saved} onUpdateConfig={async patch => { setSaved(current => ({ ...current, ...patch })); }} />;
  }
  render(<Host />);
  fireEvent.change(screen.getByLabelText('模型'), { target: { value: 'unsaved-model' } });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '切换关闭行为' })); });
  expect((screen.getByLabelText('模型') as HTMLInputElement).value).toBe('unsaved-model');
  expect(screen.queryByText('已保存')).toBeNull();
});
