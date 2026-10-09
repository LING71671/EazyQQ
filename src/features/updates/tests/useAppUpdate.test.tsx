import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useAppUpdate } from '../useAppUpdate';

const mocks = vi.hoisted(() => ({ check:vi.fn(), install:vi.fn(), listen:vi.fn(), unlisten:vi.fn() }));
vi.mock('@/api/client', () => ({ api:{ checkAppUpdate:mocks.check, upgradeApp:mocks.install, onAppUpdateProgress:mocks.listen } }));
const info = { hasUpdate:true, downloadUrl:'https://github.com/LING71671/EazyQQ/releases/download/v0.5.1/EazyQQ_0.5.1_x64-setup.exe', checksumSha256:'a'.repeat(64) };
beforeEach(() => { vi.resetAllMocks(); mocks.check.mockResolvedValue({success:true,data:info}); mocks.listen.mockResolvedValue(mocks.unlisten); });
afterEach(cleanup);

it('removes stale release data when a later check fails', async () => {
  const {result}=renderHook(useAppUpdate);
  await act(() => result.current.check());
  expect(result.current.info).toEqual(info);
  mocks.check.mockRejectedValueOnce(new Error('HTTP 429'));
  await act(() => result.current.check());
  expect(result.current.info).toBeNull(); expect(result.current.error).toBe('HTTP 429');
});

it('blocks duplicate installation while progress is active and releases the listener', async () => {
  let complete!: (value:unknown)=>void;
  mocks.install.mockImplementation(() => new Promise(resolve => {complete=resolve;}));
  const {result}=renderHook(useAppUpdate);
  await act(() => result.current.check());
  let operation!:Promise<void>;
  await act(async () => {operation=result.current.install(); await Promise.resolve();});
  await act(() => result.current.install());
  expect(mocks.install).toHaveBeenCalledTimes(1);
  act(() => mocks.listen.mock.calls[0][0]({phase:'downloading',downloadedBytes:50,totalBytes:100}));
  expect(result.current.progress?.downloadedBytes).toBe(50);
  await act(async () => {complete({success:true,data:'Installer started'});await operation;});
  expect(result.current.busy).toBeNull(); expect(mocks.unlisten).toHaveBeenCalledTimes(1);
});

it('permits retry after download failure without leaving a success spinner', async () => {
  mocks.install.mockRejectedValueOnce(new Error('SHA256 mismatch')).mockResolvedValueOnce({success:true,data:'Started'});
  const {result}=renderHook(useAppUpdate);
  await act(() => result.current.check()); await act(() => result.current.install());
  expect(result.current.error).toBe('SHA256 mismatch'); expect(result.current.progress).toBeNull();
  await act(() => result.current.install());
  expect(result.current.error).toBeNull(); expect(result.current.notice).toBe('Started'); expect(result.current.busy).toBeNull();
});

it('does not install after unmount while waiting for event subscription', async () => {
  let subscribe!: (value:()=>void)=>void;
  mocks.listen.mockImplementation(() => new Promise(resolve => {subscribe=resolve;}));
  const {result,unmount}=renderHook(useAppUpdate);
  await act(() => result.current.check());
  let operation!:Promise<void>;
  act(() => {operation=result.current.install();}); unmount();
  await act(async () => {subscribe(mocks.unlisten);await operation;});
  expect(mocks.install).not.toHaveBeenCalled(); expect(mocks.unlisten).toHaveBeenCalled();
});
