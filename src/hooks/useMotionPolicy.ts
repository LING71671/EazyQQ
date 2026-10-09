import { useEffect, useState } from 'react';

export function useMotionPolicy() {
  const [reduced, setReduced] = useState(() => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false);
  useEffect(() => {
    const preference = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    const sync = () => {
      const next = preference?.matches ?? false;
      setReduced(next);
      document.documentElement.dataset.motionPaused = String(document.hidden || next);
    };
    sync();
    preference?.addEventListener('change', sync);
    document.addEventListener('visibilitychange', sync);
    return () => {
      preference?.removeEventListener('change', sync);
      document.removeEventListener('visibilitychange', sync);
      delete document.documentElement.dataset.motionPaused;
    };
  }, []);
  return reduced;
}
