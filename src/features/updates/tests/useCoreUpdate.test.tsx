import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useCoreUpdate } from '../useCoreUpdate';
const mocks=vi.hoisted(() => ({version:vi.fn(),check:vi.fn(),install:vi.fn(),listen:vi.fn(),unlisten:vi.fn()}));
vi.mock('@/api/client',()=>({api:{getNapCatVersion:mocks.version,checkNapCatUpdate:mocks.check,upgradeNapCat:mocks.install,onNapCatUpdateProgress:mocks.listen}}));
const info={currentVersion:'4.18.28',latestVersion:'4.18.33',hasUpdate:true,status:'available',downloadUrl:'https://github.com/NapNeko/NapCatQQ/releases/download/v4.18.33/NapCat.Shell.zip'};
beforeEach(()=>{vi.resetAllMocks();mocks.version.mockResolvedValue({success:true,data:'4.18.28'});mocks.check.mockResolvedValue({success:true,data:info});mocks.listen.mockResolvedValue(mocks.unlisten);});
afterEach(cleanup);

it('ignores an initial version read that completes after an explicit update check',async()=>{
  let complete!: (value: {success:boolean;data:string}) => void;
  mocks.version.mockReturnValueOnce(new Promise(resolve=>{complete=resolve;}));
  const {result}=renderHook(useCoreUpdate);
  await act(()=>result.current.check());
  await act(async()=>{complete({success:true,data:'stale-initial-version'});});
  expect(result.current.version).toBe(info.currentVersion);
});

it('ignores the abandoned StrictMode effect after its replacement version read',async()=>{
  let complete!: (value: {success:boolean;data:string}) => void;
  mocks.version.mockReturnValueOnce(new Promise(resolve=>{complete=resolve;})).mockResolvedValueOnce({success:true,data:'fresh-version'});
  const {result}=renderHook(useCoreUpdate,{reactStrictMode:true});
  await act(async()=>{});
  expect(result.current.version).toBe('fresh-version');
  await act(async()=>{complete({success:true,data:'stale-version'});});
  expect(result.current.version).toBe('fresh-version');
});

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
  expect(result.current.notice).toContain('版本尚未确认');expect(result.current.notice).not.toContain('Core updated');
  expect(mocks.unlisten).toHaveBeenCalledTimes(1);
});

it('keeps deployment distinct from version confirmation while the version query is pending or fails',async()=>{
  mocks.install.mockResolvedValue({success:true,data:'Core updated'});
  const {result}=renderHook(useCoreUpdate);await act(()=>result.current.check());
  let reject!: (error: Error) => void;
  mocks.version.mockReturnValue(new Promise((_resolve, fail)=>{reject=fail;}));
  let pending!: Promise<void>;
  await act(async()=>{pending=result.current.install();await Promise.resolve();await Promise.resolve();});
  expect(result.current.notice).toContain('核心文件已部署');expect(result.current.notice).toContain('版本尚未确认');
  expect(result.current.version).toBe('unknown');expect(result.current.info?.status).toBe('version_unknown');
  await act(async()=>{reject(new Error('Version service unavailable'));await pending;});
  expect(result.current.notice).not.toContain('更新完成');expect(result.current.error).toContain('Version service unavailable');
  expect(result.current.version).toBe('unknown');expect(result.current.info?.hasUpdate).toBe(true);expect(result.current.busy).toBeNull();
});

it('does not retain the previous known version when the post-deployment query returns an error response',async()=>{
  mocks.install.mockResolvedValue({success:true,data:'Core updated'});
  const {result}=renderHook(useCoreUpdate);await act(()=>result.current.check());
  mocks.version.mockResolvedValue({success:false,error:{message:'Version record missing'}});
  await act(()=>result.current.install());
  expect(result.current.version).toBe('unknown');expect(result.current.info?.status).toBe('version_unknown');
  expect(result.current.error).toContain('版本读取失败');expect(result.current.notice).toContain('版本尚未确认');
});
