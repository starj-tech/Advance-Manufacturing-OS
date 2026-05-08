import type { ReactNode } from 'react';
import { tokens } from './tokens';

export type StatusKind = 'success' | 'warning' | 'danger' | 'neutral' | 'info';

export interface StatusPillProps {
  kind?: StatusKind;
  children: ReactNode;
}

const COLORS: Record<StatusKind, { bg: string; fg: string }> = {
  success: { bg: 'rgba(16, 185, 129, 0.15)', fg: tokens.colors.success },
  warning: { bg: 'rgba(245, 158, 11, 0.15)', fg: tokens.colors.warning },
  danger: { bg: 'rgba(239, 68, 68, 0.15)', fg: tokens.colors.danger },
  info: { bg: 'rgba(59, 130, 246, 0.15)', fg: tokens.colors.accent },
  neutral: { bg: 'rgba(160, 160, 160, 0.15)', fg: tokens.colors.fgMuted },
};

export function StatusPill({ kind = 'neutral', children }: StatusPillProps) {
  const c = COLORS[kind];
  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        padding: '2px 8px',
        background: c.bg,
        color: c.fg,
        borderRadius: 999,
        fontSize: 12,
        fontWeight: 500,
        textTransform: 'uppercase',
        letterSpacing: '0.04em',
      }}
    >
      <span
        style={{ width: 6, height: 6, borderRadius: '50%', background: c.fg }}
        aria-hidden
      />
      {children}
    </span>
  );
}
