import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useMotionPolicy } from '../useMotionPolicy';
import { usePresence } from '../usePresence';

let change: (() => void) | undefined;
let hidden = false;
const media = { matches: false, addEventListener: vi.fn((_type, callback) => { change = callback; }), removeEventListener: vi.fn() };
const originalHidden = Object.getOwnPropertyDescriptor(document, 'hidden');
beforeEach(() => {
  vi.useFakeTimers(); hidden = false; media.matches = false; change = undefined;
  vi.stubGlobal('matchMedia', vi.fn(() => media));
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
});
afterEach(() => {
  cleanup(); vi.useRealTimers(); vi.unstubAllGlobals();
  if (originalHidden) Object.defineProperty(document, 'hidden', originalHidden);
  else Reflect.deleteProperty(document, 'hidden');
});

it('pauses background loops while hidden and resumes when the window returns', () => {
  const { unmount } = renderHook(() => useMotionPolicy());
  expect(document.documentElement.dataset.motionPaused).toBe('false');
  act(() => { hidden = true; document.dispatchEvent(new Event('visibilitychange')); });
  expect(document.documentElement.dataset.motionPaused).toBe('true');
  act(() => { hidden = false; document.dispatchEvent(new Event('visibilitychange')); });
  expect(document.documentElement.dataset.motionPaused).toBe('false');
  unmount();
  expect(document.documentElement.dataset.motionPaused).toBeUndefined();
});

it('responds to reduced-motion changes without restarting the application', () => {
  const { result } = renderHook(() => useMotionPolicy());
  act(() => { media.matches = true; change?.(); });
  expect(result.current).toBe(true);
  expect(document.documentElement.dataset.motionPaused).toBe('true');
});

it('cancels a closing animation when the drawer is reopened quickly', () => {
  const { result, rerender } = renderHook(({ open }) => usePresence(open), { initialProps: { open: true } });
  rerender({ open: false });
  act(() => { vi.advanceTimersByTime(80); });
  rerender({ open: true });
  act(() => { vi.advanceTimersByTime(200); });
  expect(result.current).toBe(true);
  rerender({ open: false });
  act(() => { vi.advanceTimersByTime(160); });
  expect(result.current).toBe(false);
});

it('removes closed overlays immediately under reduced motion', () => {
  media.matches = true;
  const { result, rerender } = renderHook(({ open }) => usePresence(open), { initialProps: { open: true } });
  rerender({ open: false });
  expect(result.current).toBe(false);
});
