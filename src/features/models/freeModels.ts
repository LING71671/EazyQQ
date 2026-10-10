import type { FreeModelCheck } from '@/api/contracts';

export const FREE_MODEL_STATES: Record<FreeModelCheck['state'], string> = {
  candidate: '待验证',
  available: '免凭据可用',
  retired: '明确停用',
  removed_from_catalogue: '目录已移除',
  unconfirmed: '暂未确认',
  requires_conditions: '有额外条件',
};

export function nativeModelId(value: string): string {
  return value.trim().replace(/^opencode\//, '');
}
