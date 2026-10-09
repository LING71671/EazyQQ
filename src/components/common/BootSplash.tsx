import React, { useEffect, useState } from 'react';
import logoUrl from '@/assets/logo.svg';

interface BootSplashProps {
  /** When false the splash fades out and then unmounts itself. */
  visible: boolean;
  /** Human-readable description of what the app is currently waiting on. */
  stageLabel?: string;
  reducedMotion?: boolean;
}

/**
 * Full-screen boot overlay.
 *
 * Visually identical to the pre-React splash in `index.html` on purpose: the static
 * splash hands over to this component without a visible jump, and it keeps animating
 * while the first IPC round-trips (SQLite config, contacts, protocol status) settle.
 */
export const BootSplash: React.FC<BootSplashProps> = ({
  visible,
  stageLabel = '正在初始化本地运行环境',
  reducedMotion = false,
}) => {
  const [mounted, setMounted] = useState(true);
  const [opaque, setOpaque] = useState(true);

  useEffect(() => {
    if (visible) {
      setMounted(true);
      // Defer one frame so the browser actually animates the fade-in.
      const raf = requestAnimationFrame(() => setOpaque(true));
      return () => cancelAnimationFrame(raf);
    }

    setOpaque(false);
    if (reducedMotion) { setMounted(false); return; }
    const timer = setTimeout(() => setMounted(false), 180);
    return () => clearTimeout(timer);
  }, [visible, reducedMotion]);

  if (!mounted) return null;

  return (
    <div
      aria-hidden={!visible}
      className={`fixed inset-0 z-50 flex flex-col items-center justify-center gap-5 bg-slate-50 transition-opacity duration-150 ${
        opaque ? 'opacity-100' : 'opacity-0 pointer-events-none'
      }`}
    >
      <div className="relative w-[52px] h-[52px]">
        <span
          className="boot-anim-spin absolute rounded-[22px] border-2 border-sky-600/15 border-t-sky-600 border-r-sky-400"
          style={{ inset: '-7px', animation: 'boot-spin 0.9s linear infinite' }}
        />
        <div className="w-full h-full rounded-2xl bg-white border border-slate-200 flex items-center justify-center shadow-xs">
          <img src={logoUrl} alt="EazyQQ" className="w-[30px] h-[30px] block" />
        </div>
      </div>

      <div className="text-[13px] font-semibold text-slate-900 tracking-tight">EazyQQ</div>

      <div className="w-[164px] h-[3px] rounded-full bg-slate-200 overflow-hidden">
        <span
          className="boot-anim-slide block w-[42%] h-full rounded-full bg-sky-600"
          style={{ animation: 'boot-slide 1.35s cubic-bezier(0.65, 0, 0.35, 1) infinite' }}
        />
      </div>

      <div className="text-[11px] font-medium text-slate-400 tracking-wide">{stageLabel}</div>
    </div>
  );
};
