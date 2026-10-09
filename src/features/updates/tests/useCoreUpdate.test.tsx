import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useCoreUpdate } from '../useCoreUpdate';
const mocks=vi.hoisted(() => ({version:vi.fn(),check:vi.fn(),install:vi.fn(),listen:vi.fn(),unlisten:vi.fn()}));
vi.mock('@/api/client',()=>({api:{getNapCatVersion:mocks.version,checkNapCatUpdate:mocks.check,upgradeNapCat:mocks.install,onNapCatUpdateProgress:mocks.listen}}));
const info={currentVersion:'4.18.28',latestVersion:'4.18.33',hasUpdate:true,status:'available',downloadUrl:'https://github.com/NapNeko/NapCatQQ/releases/download/v4.18.33/NapCat.Shell.zip'};
beforeEach(()=>{vi.resetAllMocks();mocks.version.mockResolvedValue({success:true,data:'4.18.28'});mocks.check.mockResolvedValue({success:true,data:info});mocks.listen.mockResolvedValue(mocks.unlisten);});
afterEach(cleanup);

it('preserves the installed version on failure and allows a later verified retry',async()=>{
  mocks.install.mockRejectedValueOnce(new Error('Account remains online')).mockResolvedValueOnce({success:true,data:'Core updated'});
  const {result}=renderHook(useCoreUpdate);
  await act(()=>result.current.check());await act(()=>result.current.install());
  expect(result.current.version).toBe('4.18.28');expect(result.current.info?.hasUpdate).toBe(true);expect(result.current.busy).toBeNull();
  mocks.version.mockResolvedValue({success:true,data:'4.18.33'});
  await act(()=>result.current.install());
  expect(result.current.version).toBe('4.18.33');expect(result.current.info?.hasUpdate).toBe(false);expect(result.current.error).toBeNull();
});

it('does not claim latest when post-install version cannot be confirmed',async()=>{
  mocks.install.mockResolvedValue({success:true,data:'Core updated'});
  const {result}=renderHook(useCoreUpdate);await act(()=>result.current.check());
  mocks.version.mockResolvedValue({success:true,data:'unknown'});
  await act(()=>result.current.install());
  expect(result.current.info?.status).toBe('version_unknown');expect(result.current.info?.hasUpdate).toBe(true);
  expect(mocks.unlisten).toHaveBeenCalledTimes(1);
});
