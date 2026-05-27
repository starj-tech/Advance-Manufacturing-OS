import type { ReactNode } from 'react';

export interface ShellLayoutProps {
  title: string;
  subtitle?: string;
  sidebar?: ReactNode;
  children: ReactNode;
}

export function ShellLayout({ title, subtitle, sidebar, children }: ShellLayoutProps) {
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: sidebar ? '220px 1fr' : '1fr',
        height: '100%',
      }}
    >
      {sidebar ? (
        <aside
          style={{
            borderRight: '1px solid var(--aether-border)',
            padding: 16,
            display: 'flex',
            flexDirection: 'column',
            gap: 16,
            background: 'var(--aether-bg-elevated)',
          }}
        >
          <header>
            <h2
              style={{
                margin: 0,
                fontSize: 14,
                letterSpacing: '0.08em',
                textTransform: 'uppercase',
              }}
            >
              {title}
            </h2>
            {subtitle ? (
              <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 12 }}>
                {subtitle}
              </p>
            ) : null}
          </header>
          {sidebar}
        </aside>
      ) : null}
      <section style={{ padding: 24, overflow: 'auto' }}>{children}</section>
    </div>
  );
}
