import { expect, it } from 'vitest';
import { sanitizeEvent } from './privacy';

it('retains technical evidence but drops chat, credentials and identities', () => {
  const clean = sanitizeEvent({ type: undefined, message: 'private chat', user: { id: '10001' },
    request: { headers: { Authorization: 'secret-key' } }, extra: { token: 'secret-key' }, tags: { component: 'renderer', uin: '10001' },
    exception: { values: [{ type: 'Error', value: 'private chat', stacktrace: { frames: [{ filename: 'C:/Users/private-user/src/App.tsx', lineno: 20, vars: { token: 'secret-key' } }] } }] } });
  for (const secret of ['private chat', '10001', 'secret-key', 'private-user']) expect(JSON.stringify(clean)).not.toContain(secret);
  expect(clean.tags?.component).toBe('renderer');
  expect(clean.exception?.values?.[0].stacktrace?.frames?.[0].lineno).toBe(20);
});
