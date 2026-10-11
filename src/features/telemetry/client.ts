import * as Sentry from '@sentry/react';
import { invoke as nativeInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import { sanitizeEvent } from './privacy';

let enabled = false;
let pending = true;
const earlyErrors: Array<{ error: Error; operation: string }> = [];
export async function initializeTelemetry() {
  const dsn = import.meta.env.VITE_EAZYQQ_SENTRY_DSN;
  if (!dsn) { pending = false; return; }
  let allowed = import.meta.env.VITE_EAZYQQ_TELEMETRY === '1';
  if ('__TAURI_INTERNALS__' in window) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const status = await Promise.race([nativeInvoke<{ success: boolean; data?: { enabled: boolean } }>('get_telemetry_status'),
        new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Telemetry status timeout')), 3000); })]);
      allowed = status.success && status.data?.enabled === true;
    } catch { allowed = false; }
    finally { if (timer) clearTimeout(timer); }
  }
  if (!allowed) { pending = false; earlyErrors.length = 0; return; }
  try { Sentry.init({
    dsn, release: __EAZYQQ_RELEASE__, environment: import.meta.env.VITE_EAZYQQ_TELEMETRY_ENV || 'development',
    sampleRate: 1, tracesSampleRate: 1,
    dataCollection: { userInfo: false, cookies: false, httpHeaders: false, httpBodies: [], urlQueryParams: false,
      graphQL: { document: false, variables: false }, genAI: { inputs: false, outputs: false }, databaseQueryData: false,
      queues: false, stackFrameVariables: false, frameContextLines: 0 },
    integrations: defaults => defaults.filter(integration => ['GlobalHandlers', 'Dedupe', 'LinkedErrors', 'BrowserSession'].includes(integration.name)),
    beforeSend: sanitizeEvent,
    beforeBreadcrumb: () => null,
    beforeSendLog: log => ({ ...log, body: 'EazyQQ operation', attributes: { ...Object.fromEntries(Object.entries(log.attributes || {}).filter(([key]) => ['operation', 'component', 'duration_ms', 'status'].includes(key))), 'sentry.release': __EAZYQQ_RELEASE__ } }),
    beforeSendMetric: metric => ({ ...metric, attributes: { ...Object.fromEntries(Object.entries(metric.attributes || {}).filter(([key]) => ['operation', 'component'].includes(key))), 'sentry.release': __EAZYQQ_RELEASE__ } }),
    beforeSendSpan: span => ({ ...span, name: /^[a-zA-Z0-9_.:-]{1,128}$/.test(span.name) ? span.name : 'eazyqq.operation',
      attributes: { 'sentry.op': 'ipc', component: 'renderer', 'sentry.release': __EAZYQQ_RELEASE__, 'sentry.environment': import.meta.env.VITE_EAZYQQ_TELEMETRY_ENV || 'development' }, links: undefined }),
  }); } catch { pending = false; earlyErrors.length = 0; return; }
  enabled = true;
  pending = false;
  Sentry.setTag('component', 'renderer');
  for (const item of earlyErrors.splice(0)) captureUiError(item.error, item.operation);
}

export function captureUiError(error: unknown, operation: string) {
  if (!enabled) {
    if (pending && earlyErrors.length < 8) {
      const safe = new Error('Early UI failure');
      if (error instanceof Error && typeof error.stack === 'string') safe.stack = error.stack.slice(0, 64 * 1024);
      earlyErrors.push({ error: safe, operation });
    }
    return null;
  }
  try { return Sentry.withScope(scope => { scope.setTag('operation', operation); return Sentry.captureException(error); }); }
  catch { return null; }
}

export async function observedInvoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  if (!enabled) return nativeInvoke<T>(command, args);
  return Sentry.startSpan({ name: command, op: 'ipc', attributes: { component: 'renderer' } }, async span => {
    const started = performance.now();
    let status = 'ok';
    try {
      const result = await nativeInvoke<T>(command, args);
      if (result && typeof result === 'object' && 'success' in result && result.success === false) {
        span.setStatus({ code: 2, message: 'domain_error' });
        status = 'domain_error';
        captureUiError(new Error(`IPC ${command} returned a domain error`), command);
      }
      return result;
    } catch (error) {
      span.setStatus({ code: 2, message: 'internal_error' });
      status = 'internal_error';
      captureUiError(error, command);
      throw error;
    } finally {
      const duration = performance.now() - started;
      try {
        Sentry.logger.info('IPC operation completed', { operation: command, component: 'renderer', duration_ms: duration, status });
        Sentry.metrics.distribution('eazyqq.ipc.duration', duration, { unit: 'millisecond', attributes: { operation: command, component: 'renderer' } });
      } catch { /* Diagnostics must not change a successful application operation. */ }
    }
  });
}

export async function testFrontendTelemetry() {
  if (!enabled) throw new Error('前端诊断上报未启用');
  const eventId = captureUiError(new Error('EazyQQ synthetic renderer diagnostic'), 'telemetry.test');
  if (!await Sentry.flush(5000)) throw new Error(`前端发送队列未在5秒内处理完成，事件编号 ${eventId}`);
  return eventId;
}
