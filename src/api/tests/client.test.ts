import { expect, it, vi } from 'vitest';
import { api } from '../client';
import { observedInvoke } from '@/features/telemetry/client';

vi.mock('@/features/telemetry/client', () => ({ observedInvoke: vi.fn().mockResolvedValue({ success: true }) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

it('includes the group destination type in the desktop send command', async () => {
  await api.sendMessage('10001', 'Group reply', 'group');
  expect(observedInvoke).toHaveBeenCalledWith('send_message', {
    targetId: '10001', content: 'Group reply', targetType: 'group',
  });
});
