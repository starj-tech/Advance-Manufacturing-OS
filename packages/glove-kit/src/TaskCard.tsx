import type { ReactNode } from 'react';
import { gloveTokens } from './tokens';

export interface TaskCardProps {
  title: string;
  subtitle?: string;
  status?: 'pending' | 'running' | 'done' | 'blocked';
  meta?: ReactNode;
  onPress?: () => void;
}

const STATUS_COLOR: Record<NonNullable<TaskCardProps['status']>, string> = {
  pending: 'var(--aether-fg-muted)',
  running: 'var(--aether-accent)',
  done: 'var(--aether-success)',
  blocked: 'var(--aether-danger)',
};

export function TaskCard({ title, subtitle, status = 'pending', meta, onPress }: TaskCardProps) {
  return (
    <button
      onClick={onPress}
      style={{
        textAlign: 'left',
        background: 'var(--aether-bg-elevated)',
        border: `2px solid var(--aether-border)`,
        borderLeft: `8px solid ${STATUS_COLOR[status]}`,
        borderRadius: gloveTokens.radiusLg,
        padding: gloveTokens.spacingLg,
        minHeight: 96,
        display: 'flex',
        flexDirection: 'column',
        gap: gloveTokens.spacingXs,
        color: 'var(--aether-fg)',
        cursor: 'pointer',
        width: '100%',
      }}
    >
      <div style={{ fontSize: gloveTokens.fontSizePrimary, fontWeight: 700 }}>{title}</div>
      {subtitle ? (
        <div style={{ fontSize: gloveTokens.fontSizeBody, color: 'var(--aether-fg-muted)' }}>
          {subtitle}
        </div>
      ) : null}
      {meta ? <div style={{ marginTop: gloveTokens.spacingSm }}>{meta}</div> : null}
    </button>
  );
}
