import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { StylePackCard } from '../settings/StylePackCard';

const mocks = vi.hoisted(() => ({ manage: vi.fn(), save: vi.fn() }));
vi.mock('@/api/client', () => ({ api: { manageStylePacks: mocks.manage } }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: mocks.save }));
const pack = { schemaVersion: 1, id: 'example', name: '通用示例', version: '1.0.0', rules: {
  sentenceLength: 'short', punctuation: 'minimal', tone: 'neutral', lineBreaks: 'natural',
  preferredPhrases: [], avoidPhrases: [], qqFaceIds: ['76'],
} };
const selected = { enabled: true, activeId: 'example', packs: [pack] };
beforeEach(() => { mocks.manage.mockReset(); mocks.save.mockReset(); });
afterEach(cleanup);

it('imports selected JSON and waits for the installed status before enabling controls', async () => {
  mocks.manage.mockResolvedValueOnce({ success: true, data: { enabled: false, activeId: null, packs: [] } });
  let finish!: (value: unknown) => void;
  mocks.manage.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<StylePackCard />);
  await act(async () => { await Promise.resolve(); });
  const file = { size: 100, text: async () => JSON.stringify(pack) };
  await act(async () => { fireEvent.change(screen.getByLabelText('风格包JSON文件'), { target: { files: [file] } }); });
  expect(mocks.manage).toHaveBeenLastCalledWith('import', JSON.stringify(pack), undefined, undefined);
  expect((screen.getByRole('button', { name: '导入' }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { finish({ success: true, data: selected }); });
  expect((screen.getByLabelText('当前风格包') as HTMLSelectElement).value).toBe('example');
  expect(screen.getByText(/已启用.*偏好短句/)).toBeTruthy();
});

it('retains the previous installed selection when a package is rejected', async () => {
  mocks.manage.mockResolvedValueOnce({ success: true, data: selected })
    .mockResolvedValueOnce({ success: false, error: { message: '风格包包含未知字段' } });
  render(<StylePackCard />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { fireEvent.change(screen.getByLabelText('风格包JSON文件'), { target: { files: [{ size: 5, text: async () => '{}' }] } }); });
  expect(screen.getByRole('alert').textContent).toContain('未知字段');
  expect((screen.getByLabelText('当前风格包') as HTMLSelectElement).value).toBe('example');
  expect(screen.getByRole('button', { name: '停用风格包' })).toBeTruthy();
});

it('does not let a late initial listing overwrite a newly installed selection', async () => {
  let finishList!: (value: unknown) => void;
  mocks.manage.mockImplementationOnce(() => new Promise(resolve => { finishList = resolve; }))
    .mockResolvedValueOnce({ success: true, data: selected });
  render(<StylePackCard />);
  await act(async () => { fireEvent.change(screen.getByLabelText('风格包JSON文件'), { target: { files: [{ size: 10, text: async () => JSON.stringify(pack) }] } }); });
  expect((screen.getByLabelText('当前风格包') as HTMLSelectElement).value).toBe('example');
  await act(async () => { finishList({ success: true, data: { enabled: false, activeId: null, packs: [] } }); });
  expect((screen.getByLabelText('当前风格包') as HTMLSelectElement).value).toBe('example');
});

it('rejects oversized files before reading and exports only after a destination is chosen', async () => {
  mocks.manage.mockResolvedValue({ success: true, data: selected });
  const read = vi.fn();
  render(<StylePackCard />);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { fireEvent.change(screen.getByLabelText('风格包JSON文件'), { target: { files: [{ size: 65_537, text: read }] } }); });
  expect(read).not.toHaveBeenCalled();
  expect(screen.getByRole('alert').textContent).toContain('64KiB');
  mocks.save.mockResolvedValueOnce(null).mockResolvedValueOnce('B:/export/example.json');
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '导出' })); });
  expect(mocks.manage).toHaveBeenCalledTimes(1);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '导出' })); });
  expect(mocks.manage).toHaveBeenLastCalledWith('export', undefined, 'example', 'B:/export/example.json');
});
