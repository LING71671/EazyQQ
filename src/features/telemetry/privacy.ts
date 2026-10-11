import type { ErrorEvent } from '@sentry/react';

const safeTags = new Set(['component', 'operation', 'operation_id', 'phase', 'error_code', 'provider']);
const identifier = (value: string) => /^[a-zA-Z0-9_.:-]{1,128}$/.test(value);
const asset = (value?: string) => value?.split(/[?#]/)[0].split(/[\\/]/).slice(-2).join('/');

export function sanitizeEvent(event: ErrorEvent): ErrorEvent {
  return {
    type: undefined,
    event_id: event.event_id, timestamp: event.timestamp, platform: 'javascript', level: event.level,
    release: event.release, environment: event.environment,
    message: 'EazyQQ runtime error; dynamic content remains in local diagnostics',
    tags: Object.fromEntries(Object.entries(event.tags || {}).filter(([key, value]) => safeTags.has(key) && identifier(String(value)))),
    contexts: event.contexts?.trace ? { trace: { trace_id: event.contexts.trace.trace_id, span_id: event.contexts.trace.span_id,
      parent_span_id: event.contexts.trace.parent_span_id, op: event.contexts.trace.op, status: event.contexts.trace.status } } : undefined,
    exception: event.exception && { values: event.exception.values?.map(value => ({
      type: value.type && identifier(value.type) ? value.type : 'RuntimeError',
      value: 'Dynamic exception details withheld', mechanism: value.mechanism && { type: value.mechanism.type, handled: value.mechanism.handled },
      stacktrace: value.stacktrace && { frames: value.stacktrace.frames?.map(frame => ({
        filename: asset(frame.filename), function: frame.function, lineno: frame.lineno, colno: frame.colno, in_app: frame.in_app,
      })) },
    })) },
  };
}
