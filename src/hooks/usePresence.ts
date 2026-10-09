import { useEffect, useState } from 'react';

/** Keep closing overlays mounted briefly, and cancel stale exits on reopening. */
export function usePresence(visible: boolean, exitMs = 160) {
  const [mounted, setMounted] = useState(visible);
  useEffect(() => {
    if (visible) { setMounted(true); return; }
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
      setMounted(false); return;
    }
    const timer = setTimeout(() => setMounted(false), exitMs);
    return () => clearTimeout(timer);
  }, [visible, exitMs]);
  return mounted;
}
