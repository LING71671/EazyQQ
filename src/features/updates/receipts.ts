import type { UpdateReceipt } from '@/api/contracts';

const phases = new Set(['checking', 'checked', 'downloading', 'verifying', 'ready', 'handoff', 'waiting_parent', 'installing', 'installer_succeeded', 'pending_verification', 'deployed', 'succeeded', 'failed', 'aborted']);
const timestamp = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value > 0 && value <= 8_640_000_000_000_000;

export function isUpdateReceipt(value: unknown): value is UpdateReceipt {
  if (!value || typeof value !== 'object') return false;
  const item = value as Record<string, unknown>;
  return item.schemaVersion === 1 && (item.component === 'app' || item.component === 'napcat')
    && typeof item.operationId === 'string' && /^[0-9a-f]{32}$/i.test(item.operationId)
    && (item.targetVersion === null || typeof item.targetVersion === 'string')
    && typeof item.phase === 'string' && phases.has(item.phase)
    && timestamp(item.startedAtMs) && timestamp(item.updatedAtMs) && item.updatedAtMs >= item.startedAtMs
    && (item.installerExitCode === null || (typeof item.installerExitCode === 'number' && Number.isInteger(item.installerExitCode)))
    && (item.error === null || typeof item.error === 'string')
    && (item.operation === undefined || item.operation === 'check' || item.operation === 'install')
    && (item.verifiedVersion === undefined || item.verifiedVersion === null || typeof item.verifiedVersion === 'string')
    && (item.events === undefined || (Array.isArray(item.events) && item.events.length <= 32 && item.events.every(event => event && typeof event === 'object'
      && phases.has(event.phase) && timestamp(event.atMs) && event.atMs >= (item.startedAtMs as number) && event.atMs <= (item.updatedAtMs as number))))
    && (!['installer_succeeded', 'pending_verification'].includes(item.phase as string) || item.installerExitCode === 0)
    && (item.phase !== 'succeeded' || (item.component === 'app' && item.installerExitCode === 0 && typeof item.verifiedVersion === 'string' && item.verifiedVersion === item.targetVersion));
}
