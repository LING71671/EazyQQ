import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';

export const AI_TEST_FINISHED = 'eazyqq:ai-test-finished';
export const notifyAiTestFinished = () => window.dispatchEvent(new Event(AI_TEST_FINISHED));
type ChainStatus = NonNullable<Awaited<ReturnType<typeof api.getChainStatus>>['data']>;

export function useChainStatus(enabled = true) {
  const [status, setStatus] = useState<ChainStatus>({ links: [], firstBreak: null, hasFailure: false, uptimeSecs: 0 });
  const [isLoading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const alive = useRef(false);
  const epoch = useRef(0);
  const inFlight = useRef(false);
  const queued = useRef(false);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const refresh = useCallback(async () => {
    if (!enabled || !alive.current) return;
    if (inFlight.current) { queued.current = true; epoch.current++; return; }
    inFlight.current = true;
    const current = ++epoch.current;
    setLoading(true);
    try {
      const result = await api.getChainStatus();
      if (!result.success || !result.data) throw new Error(result.error?.message || '链路状态读取失败');
      if (alive.current && current === epoch.current) { setStatus(result.data); setError(''); }
    } catch (error) {
      if (alive.current && current === epoch.current) setError(error instanceof Error ? error.message : String(error));
    } finally {
      inFlight.current = false;
      if (alive.current) {
        if (queued.current) { queued.current = false; void refreshRef.current(); }
        else setLoading(false);
      }
    }
  }, [enabled]);
  refreshRef.current = refresh;
  useEffect(() => {
    if (!enabled) return;
    alive.current = true;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => { await refresh(); if (!stopped) timer = setTimeout(poll, 5000); };
    const onTest = () => { void refresh(); };
    window.addEventListener(AI_TEST_FINISHED, onTest);
    void poll();
    return () => { stopped = true; alive.current = false; epoch.current++; queued.current = false; clearTimeout(timer); window.removeEventListener(AI_TEST_FINISHED, onTest); };
  }, [enabled, refresh]);
  return { ...status, isLoading, error, refresh };
}
