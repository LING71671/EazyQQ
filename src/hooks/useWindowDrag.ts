import React, { useCallback } from 'react';
import { api } from '@/api/client';

/** Controls inside a drag region must keep receiving their own clicks. */
const INTERACTIVE_SELECTOR = 'button, a, input, textarea, select, [data-no-drag]';

/**
 * Makes a frameless Tauri window draggable.
 *
 * Spread the returned handlers onto any element that also carries the
 * `data-drag-handle` attribute (the attribute itself marks the draggable area).
 *
 * Dragging is routed through the native Rust command `app_start_drag_window`
 * instead of the built-in `data-tauri-drag-region` script. Custom commands are not
 * gated by the Tauri capability ACL, so dragging keeps working even when the window
 * permissions are missing or stale - which is exactly what silently broke it before.
 * Using both paths at once is avoided on purpose: each one would post its own drag
 * request for the same click.
 */
export function useWindowDrag(onToggleMaximize?: () => void) {
  const isDragRegion = (target: HTMLElement) =>
    !!target.closest('[data-drag-handle]') && !target.closest(INTERACTIVE_SELECTOR);

  const onMouseDown = useCallback((e: React.MouseEvent<HTMLElement>) => {
    // Only the first press of a left click starts a drag; `detail > 1` belongs to a
    // double click, which is reserved for maximize / restore.
    if (e.button !== 0 || e.detail !== 1) return;
    if (!isDragRegion(e.target as HTMLElement)) return;
    api.startDragWindow().catch(() => {});
  }, []);

  const onDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLElement>) => {
      if (!isDragRegion(e.target as HTMLElement)) return;
      if (onToggleMaximize) {
        onToggleMaximize();
      } else {
        api.toggleMaximizeWindow().catch(() => {});
      }
    },
    [onToggleMaximize]
  );

  return { onMouseDown, onDoubleClick };
}
