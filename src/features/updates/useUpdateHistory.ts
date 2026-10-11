import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { UpdateReceipt } from '@/api/contracts';
import { isUpdateReceipt } from './receipts';

export function useUpdateHistory() {
  const [receipts, setReceipts] = useState<UpdateReceipt[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(false);
  const inFlight = useRef(false);
  const refresh = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setLoading(true);
    try {
      const result = await api.getUpdateHistory();
      if (!active.current) return;
      if (!result.success || !Array.isArray(result.data)) throw new Error(result.error?.message || '读取更新记录失败');
      if (!result.data.every(isUpdateReceipt)) throw new Error('更新记录格式无效，未将损坏记录显示为成功');
      setReceipts(result.data);
      setError(null);
    } catch (failure) {
      if (active.current) setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      inFlight.current = false;
      if (active.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    active.current = true;
    void refresh();
    return () => { active.current = false; };
  }, [refresh]);
  return { receipts, loading, error, refresh };
}
