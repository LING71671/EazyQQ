import { Loader2 } from 'lucide-react';
import type { QuickLoginAccountDto } from '@/api/contracts';

export function QuickLoginAccounts({ accounts, loggedIn, onLogin, switching }: {
  accounts: QuickLoginAccountDto[];
  loggedIn: boolean;
  onLogin?: (uin: string) => void;
  switching?: string | null;
}) {
  return <section aria-label="本机记忆账号">
    <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">{loggedIn ? '切换账号' : '本机记忆账号'}</h2>
    <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">{accounts.length ? '可使用已有凭据快速登录' : '暂无记忆账号，可扫码或登记账号'}</p>
    {accounts.length > 0 && <ul className="mt-3 max-h-72 overflow-y-auto divide-y divide-slate-200 dark:divide-slate-700">
      {accounts.map(account => <li key={account.uin} className="flex items-center gap-3 py-3">
        <img src={account.faceUrl || `https://q1.qlogo.cn/g?b=qq&nk=${account.uin}&s=100`} alt="" className="h-9 w-9 shrink-0 rounded-full object-cover" onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
        <div className="min-w-0 flex-1"><p className="truncate text-sm font-medium text-slate-900 dark:text-slate-100" title={account.nickname}>{account.nickname || account.uin}</p><p className="mt-0.5 text-xs tabular-nums text-slate-600 dark:text-slate-400">{account.uin}</p></div>
        <button type="button" onClick={() => onLogin?.(account.uin)} disabled={!!switching || !onLogin} aria-label={`${loggedIn ? '切换登录' : '一键登录'} ${account.uin}`} className="inline-flex shrink-0 items-center gap-2 rounded-lg bg-sky-600 px-3 py-2 text-sm font-medium text-white hover:bg-sky-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:cursor-not-allowed disabled:opacity-60">
          {switching === account.uin && <Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />}
          {switching === account.uin ? '登录中…' : loggedIn ? '切换登录' : '一键登录'}
        </button>
      </li>)}
    </ul>}
  </section>;
}
