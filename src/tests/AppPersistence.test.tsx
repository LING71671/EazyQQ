import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { App } from '../App';
import { api } from '@/api/client';
import type { AppConfig, GroupSummaryDto } from '@/api/contracts';

const mocks = vi.hoisted(() => ({
  chat: { contacts: [], drafts: [], chatMessages: [], selectedChatContact: null, isChatDrawerOpen: false,
    setContacts: vi.fn(), setDrafts: vi.fn(), fetchContactsAndDrafts: vi.fn(), clearActionError: vi.fn() },
  protocol: { protocolStatus: {}, setProtocolStatus: vi.fn() },
  files: { selectedGroupId: '', files: [], setFiles: vi.fn(), setSelectedGroupId: vi.fn() },
}));
vi.mock('@/hooks/useChatManager', () => ({ useChatManager: () => mocks.chat }));
vi.mock('@/hooks/useProtocolState', () => ({ useProtocolState: () => mocks.protocol }));
vi.mock('@/hooks/useGroupFilesManager', () => ({ useGroupFilesManager: () => mocks.files }));
vi.mock('@/hooks/useMotionPolicy', () => ({ useMotionPolicy: () => true }));
vi.mock('@/api/client', () => ({ api: {
  showWindow: vi.fn(), getContacts: vi.fn(), getSummaryHistory: vi.fn(), getConfig: vi.fn(), getPendingDrafts: vi.fn(),
  checkDependencies: vi.fn(), updateConfig: vi.fn(), deleteSummary: vi.fn(),
} }));
vi.mock('@/components/layout/Sidebar', () => ({ Sidebar: ({ onSelectView }: { onSelectView: (view: string) => void }) =>
  <><button onClick={() => onSelectView('settings')}>Settings</button><button onClick={() => onSelectView('summaries')}>Summaries</button></> }));
vi.mock('@/components/layout/TopHeader', () => ({ TopHeader: () => null }));
vi.mock('@/components/common/BootSplash', () => ({ BootSplash: () => null }));
vi.mock('@/components/manual/ManualDrawer', () => ({ ManualDrawer: () => null }));
vi.mock('@/components/health/ChainHealthDrawer', () => ({ ChainHealthDrawer: () => null }));
vi.mock('@/components/chat/ChatDrawer', () => ({ ChatDrawer: () => null }));
vi.mock('@/views/LoginView', () => ({ LoginView: () => null }));
vi.mock('@/views/ContactsView', () => ({ ContactsView: () => null }));
vi.mock('@/views/DraftsView', () => ({ DraftsView: () => null }));
vi.mock('@/views/FilesView', () => ({ FilesView: () => null }));
vi.mock('@/views/SummariesView', () => ({ SummariesView: ({ summaries, onDeleteSummary }: {
  summaries: GroupSummaryDto[]; onDeleteSummary: (id: string) => void;
}) => <>{summaries.map(summary => <button key={summary.id} onClick={() => onDeleteSummary(summary.id)}>Delete {summary.id}</button>)}</> }));
vi.mock('@/views/SettingsView', () => ({ SettingsView: ({ config, onUpdateConfig }: {
  config: AppConfig; onUpdateConfig: (patch: Partial<AppConfig>) => Promise<void>;
}) => <><output aria-label="Active model">{config.ai.model}</output><button onClick={() => {
  void onUpdateConfig({ ai: { ...config.ai, model: 'new-model' } }).catch(() => {});
}}>Save config</button></> }));

const failure = { success: false, timestamp: 0, error: { code: 1001, message: 'Storage unavailable' } };
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.showWindow).mockResolvedValue(undefined);
  vi.mocked(api.getContacts).mockResolvedValue({ success: true, timestamp: 0, data: { list: [], total: 0 } });
  vi.mocked(api.getSummaryHistory).mockResolvedValue({ success: true, timestamp: 0, data: [{ id: 'summary-1' } as GroupSummaryDto] });
  vi.mocked(api.getConfig).mockResolvedValue({ success: true, timestamp: 0, data: { ai: { model: 'saved-model' } } as AppConfig });
  vi.mocked(api.getPendingDrafts).mockResolvedValue({ success: true, timestamp: 0, data: [] });
  vi.mocked(api.checkDependencies).mockResolvedValue({ success: false, timestamp: 0 });
});
afterEach(cleanup);

it('does not publish edited config before persistence and retains the active model on refusal', async () => {
  let finish!: (value: Awaited<ReturnType<typeof api.updateConfig>>) => void;
  vi.mocked(api.updateConfig).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  await act(async () => { render(<App />); });
  fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save config' }));
  expect(screen.getByLabelText('Active model').textContent).toBe('saved-model');
  await act(async () => { finish(failure); });
  expect(screen.getByLabelText('Active model').textContent).toBe('saved-model');
});

it('publishes config after confirmed persistence', async () => {
  vi.mocked(api.updateConfig).mockImplementation(async config => ({ success: true, timestamp: 0, data: config as AppConfig }));
  await act(async () => { render(<App />); });
  fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Save config' })); });
  expect(screen.getByLabelText('Active model').textContent).toBe('new-model');
});

it('keeps a summary and displays the cause when deletion is refused', async () => {
  vi.mocked(api.deleteSummary).mockResolvedValue(failure);
  await act(async () => { render(<App />); });
  fireEvent.click(screen.getByRole('button', { name: 'Summaries' }));
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Delete summary-1' })); });
  expect(screen.getByRole('button', { name: 'Delete summary-1' })).toBeTruthy();
  expect(screen.getByRole('alert').textContent).toContain('Storage unavailable');
});
