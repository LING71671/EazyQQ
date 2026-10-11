import type { UpdateReceipt } from '@/api/contracts';

const phases = new Set(['handoff', 'waiting_parent', 'verifying', 'installing', 'installer_succeeded', 'failed']);
const timestamp = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value > 0 && value <= 8_640_000_000_000_000;

export function isUpdateReceipt(value: unknown): value is UpdateReceipt {
  if (!value || typeof value !== 'object') return false;
  const item = value as Record<string, unknown>;
  return item.schemaVersion === 1 && item.component === 'app'
    && typeof item.operationId === 'string' && /^[0-9a-f]{32}$/i.test(item.operationId)
    && (item.targetVersion === null || typeof item.targetVersion === 'string')
    && typeof item.phase === 'string' && phases.has(item.phase)
    && timestamp(item.startedAtMs) && timestamp(item.updatedAtMs) && item.updatedAtMs >= item.startedAtMs
    && (item.installerExitCode === null || (typeof item.installerExitCode === 'number' && Number.isInteger(item.installerExitCode)))
    && (item.error === null || typeof item.error === 'string')
    && (item.phase !== 'installer_succeeded' || item.installerExitCode === 0);
}
