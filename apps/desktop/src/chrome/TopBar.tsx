import { useNavigate } from 'react-router-dom';
import { SHELLS, useShellStore } from '@aether/shell-runtime';
import { StatusPill } from '@aether/ui-kit';
import { useLocale } from '@aether/i18n';
import { useSession } from '../core/session-provider';
import { useShellSwitcher } from '../core/shell-switcher';

export function TopBar() {
  const { session, signOut } = useSession();
  const activeShell = useShellStore((s) => s.activeShell);
  const notifications = useShellStore((s) => s.notifications);
  const navigate = useNavigate();
  const { switchShell } = useShellSwitcher();
  const { locale, setLocale, available } = useLocale();

  if (!session) return null;

  const allowedShells = SHELLS.filter((s) => {
    // For PR #1: developer can see all; everyone else only their primary role.
    if (session.permissions.includes('*')) return true;
    return s.id === session.primaryRole;
  });

  return (
    <header
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '8px 16px',
        borderBottom: '1px solid var(--aether-border)',
        background: 'var(--aether-bg-elevated)',
        height: 56,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <span
          style={{
            fontSize: 14,
            fontWeight: 700,
            letterSpacing: '0.1em',
            color: 'var(--aether-fg)',
          }}
        >
          AETHER-OS
        </span>
        <nav style={{ display: 'flex', gap: 4 }}>
          {allowedShells.map((s) => {
            const active = s.id === activeShell;
            return (
              <button
                key={s.id}
                onClick={() => switchShell(s.id)}
                style={{
                  padding: '6px 12px',
                  background: active ? 'var(--aether-accent)' : 'transparent',
                  color: active ? '#fff' : 'var(--aether-fg-muted)',
                  border: 'none',
                  borderRadius: 6,
                  fontSize: 13,
                  fontWeight: 500,
                }}
              >
                {s.label}
              </button>
            );
          })}
        </nav>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <StatusPill kind="warning">offline · sync paused</StatusPill>
        <select
          value={locale.tag}
          onChange={(e) => setLocale(e.target.value)}
          aria-label="Language and currency"
          style={{
            background: 'var(--aether-bg)',
            color: 'var(--aether-fg)',
            border: '1px solid var(--aether-border)',
            borderRadius: 6,
            padding: '5px 8px',
            fontSize: 13,
          }}
        >
          {available.map((l) => (
            <option key={l.tag} value={l.tag}>
              {l.nativeName} · {l.defaultCurrency}
            </option>
          ))}
        </select>
        <button
          onClick={() => navigate('/_notifications')}
          aria-label={`${notifications} notifications`}
          style={{
            background: 'transparent',
            border: '1px solid var(--aether-border)',
            color: 'var(--aether-fg)',
            borderRadius: 6,
            padding: '6px 10px',
            fontSize: 13,
          }}
        >
          🔔 {notifications}
        </button>
        <span style={{ fontSize: 13, color: 'var(--aether-fg-muted)' }}>
          {session.displayName}
        </span>
        <button
          onClick={signOut}
          style={{
            background: 'transparent',
            border: '1px solid var(--aether-border)',
            color: 'var(--aether-fg-muted)',
            borderRadius: 6,
            padding: '6px 10px',
            fontSize: 13,
          }}
        >
          Sign out
        </button>
      </div>
    </header>
  );
}
