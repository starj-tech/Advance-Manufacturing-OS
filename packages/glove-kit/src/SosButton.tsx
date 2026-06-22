import { useEffect, useRef, useState } from 'react';
import { gloveTokens } from './tokens';

export interface SosButtonProps {
  /** Hold duration in milliseconds before SOS fires. Default 2s. */
  holdMs?: number;
  /** Called when the user has held the button for `holdMs`. */
  onTrigger: () => void;
  /** Called when the hold is canceled before completion. */
  onCancel?: () => void;
  disabled?: boolean;
}

/**
 * Glove-friendly SOS button. Press-and-hold (default 2s) to avoid
 * accidental triggers. Visual + haptic + audible feedback should be
 * triple-redundant per safety guidelines.
 */
export function SosButton({ holdMs = 2000, onTrigger, onCancel, disabled }: SosButtonProps) {
  const [progress, setProgress] = useState(0);
  const startedAt = useRef<number | null>(null);
  const raf = useRef<number | null>(null);
  const fired = useRef(false);

  useEffect(() => {
    return () => {
      if (raf.current !== null) cancelAnimationFrame(raf.current);
    };
  }, []);

  const tick = () => {
    if (startedAt.current === null) return;
    const elapsed = Date.now() - startedAt.current;
    const p = Math.min(elapsed / holdMs, 1);
    setProgress(p);
    if (p >= 1 && !fired.current) {
      fired.current = true;
      onTrigger();
      return;
    }
    raf.current = requestAnimationFrame(tick);
  };

  const start = () => {
    if (disabled) return;
    fired.current = false;
    startedAt.current = Date.now();
    raf.current = requestAnimationFrame(tick);
  };

  const cancel = () => {
    if (raf.current !== null) cancelAnimationFrame(raf.current);
    if (startedAt.current !== null && !fired.current) {
      onCancel?.();
    }
    startedAt.current = null;
    setProgress(0);
  };

  return (
    <button
      onPointerDown={start}
      onPointerUp={cancel}
      onPointerLeave={cancel}
      onPointerCancel={cancel}
      disabled={disabled}
      aria-label="SOS — hold to trigger emergency alert"
      style={{
        position: 'relative',
        width: 200,
        height: 200,
        borderRadius: '50%',
        background: 'var(--aether-danger)',
        color: '#fff',
        fontSize: 36,
        fontWeight: 800,
        letterSpacing: '0.1em',
        border: '6px solid rgba(255,255,255,0.2)',
        boxShadow: `0 0 0 ${4 + progress * 20}px rgba(239, 68, 68, ${0.2 + progress * 0.3})`,
        cursor: disabled ? 'not-allowed' : 'pointer',
        transition: 'box-shadow 80ms linear',
        userSelect: 'none',
        touchAction: 'manipulation',
      }}
    >
      SOS
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 24,
          fontSize: gloveTokens.fontSizeBody,
          fontWeight: 500,
          opacity: 0.9,
        }}
      >
        {progress > 0 ? `Hold… ${Math.round(progress * 100)}%` : 'Hold to alert'}
      </div>
    </button>
  );
}
