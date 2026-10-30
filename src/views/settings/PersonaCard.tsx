import { useEffect, useRef, useState } from 'react';
import { api } from '@/api/client';
import type { ContactItemDto } from '@/api/contracts';

const fieldClass = 'rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-2 text-sm text-slate-800 dark:text-slate-200 focus:border-sky-500 focus:outline-none disabled:opacity-50';

export function PersonaCard({ accountUin, contacts = [] }: { accountUin?: string; contacts?: ContactItemDto[] }) {
  const [scope, setScope] = useState('account');
  const [targetId, setTargetId] = useState('');
  const [memberUin, setMemberUin] = useState('');
  const [text, setText] = useState('');
  const [inherited, setInherited] = useState('');
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const generation = useRef(0);
  const key = `${accountUin || ''}:${scope}:${targetId}:${memberUin}`;
  const currentKey = useRef(key);
  currentKey.current = key;
  const valid = !!accountUin && (scope === 'account' || !!targetId) && (scope !== 'member' || /^[1-9]\d{4,19}$/.test(memberUin));
  const requestScope = { ...(scope === 'account' ? {} : { targetType: scope === 'private' ? 'private' : 'group', targetId }), ...(scope === 'member' ? { memberUin } : {}) };

  useEffect(() => {
    const request = ++generation.current;
    setText(''); setInherited(''); setRevision(0); setError(null); setNotice(null); setBusy(false);
    if (!valid) return;
    setBusy(true);
    api.managePersonas({ operation: 'get', ...requestScope }).then(response => {
      if (request !== generation.current || currentKey.current !== key) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '读取表达偏好失败');
      setText(response.data.text); setInherited(response.data.inheritedText); setRevision(response.data.revision);
    }).catch(reason => { if (request === generation.current && currentKey.current === key) setError(reason instanceof Error ? reason.message : String(reason)); })
      .finally(() => { if (request === generation.current && currentKey.current === key) setBusy(false); });
    return () => { generation.current++; };
  }, [key, valid]);

  const save = async (operation: 'set' | 'clear') => {
    if (!valid || busy) return;
    const request = ++generation.current;
    const requestKey = key;
    setBusy(true); setError(null); setNotice(null);
    try {
      const response = await api.managePersonas({ operation, ...requestScope, ...(operation === 'set' ? { text } : {}) });
      if (request !== generation.current || requestKey !== currentKey.current) return;
      if (!response.success || !response.data) throw new Error(response.error?.message || '保存表达偏好失败');
      setText(response.data.text); setInherited(response.data.inheritedText); setRevision(response.data.revision); setNotice(operation === 'clear' ? '已恢复继承' : '已保存');
    } catch (reason) { if (request === generation.current && requestKey === currentKey.current) setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { if (request === generation.current && requestKey === currentKey.current) setBusy(false); }
  };

  const available = contacts.filter(contact => scope === 'private' ? contact.targetType === 'friend' : contact.targetType === 'group');
  return <section aria-labelledby="persona-heading" aria-busy={busy} className="space-y-4 rounded-2xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-5">
    <div className="flex items-center justify-between"><h3 id="persona-heading" className="text-sm font-semibold text-slate-900 dark:text-slate-100">分层表达偏好</h3><span className="text-xs text-slate-400 dark:text-slate-500">版本 {revision}</span></div>
    <div className="flex flex-wrap gap-3">
      <label className="flex flex-col gap-1 text-xs text-slate-500 dark:text-slate-400">适用范围<select aria-label="表达偏好范围" value={scope} onChange={event => { setScope(event.target.value); setTargetId(''); setMemberUin(''); }} className={fieldClass} disabled={!accountUin}>
        <option value="account">当前账号</option><option value="private">指定好友</option><option value="group">指定群</option><option value="member">指定群成员</option>
      </select></label>
      {scope !== 'account' && <label className="flex min-w-0 flex-1 flex-col gap-1 text-xs text-slate-500 dark:text-slate-400">会话<select aria-label="表达偏好会话" value={targetId} onChange={event => setTargetId(event.target.value)} className={fieldClass}><option value="">选择会话</option>{available.map(contact => <option key={contact.id} value={contact.targetId}>{contact.name} ({contact.targetId})</option>)}</select></label>}
      {scope === 'member' && <label className="flex flex-col gap-1 text-xs text-slate-500 dark:text-slate-400">成员 QQ<input aria-label="表达偏好成员QQ" value={memberUin} onChange={event => setMemberUin(event.target.value)} inputMode="numeric" className={fieldClass} /></label>}
    </div>
    <label className="block text-xs text-slate-500 dark:text-slate-400">表达偏好<textarea aria-label="表达偏好正文" rows={4} maxLength={4000} value={text} onChange={event => { setText(event.target.value); setNotice(null); }} disabled={!valid || busy} className={`${fieldClass} mt-1 w-full resize-y`} /></label>
    {inherited && <details className="text-xs text-slate-500 dark:text-slate-400"><summary className="cursor-pointer">继承的表达偏好</summary><p className="mt-2 whitespace-pre-wrap break-words">{inherited}</p></details>}
    {error && <p role="alert" className="text-xs text-red-700 dark:text-red-300">{error}</p>}
    {notice && <p role="status" className="text-xs text-sky-700 dark:text-sky-300">{notice}</p>}
    <div className="flex gap-2"><button type="button" onClick={() => void save('set')} disabled={!valid || busy || !text.trim()} className="rounded-lg bg-sky-600 px-3 py-2 text-xs text-white hover:bg-sky-700 disabled:opacity-50">保存表达偏好</button><button type="button" onClick={() => void save('clear')} disabled={!valid || busy} className="rounded-lg border border-slate-200 dark:border-slate-700 px-3 py-2 text-xs text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 disabled:opacity-50">恢复继承</button></div>
  </section>;
}
