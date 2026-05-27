import { Navigate, useNavigate } from 'react-router-dom';
import { useTranslation } from '@aether/i18n';
import { useSession } from './session-provider';
import type { Role } from '@aether/shell-runtime';

const ROLES: Array<{ id: Role; labelKey: string; descKey: string }> = [
  { id: 'developer', labelKey: 'role.developer.label', descKey: 'role.developer.desc' },
  { id: 'executive', labelKey: 'role.executive.label', descKey: 'role.executive.desc' },
  { id: 'manager', labelKey: 'role.manager.label', descKey: 'role.manager.desc' },
  { id: 'employee', labelKey: 'role.employee.label', descKey: 'role.employee.desc' },
];

export function LoginScreen() {
  const { session, signInDevMode } = useSession();
  const { t } = useTranslation();
  const navigate = useNavigate();

  if (session) {
    return <Navigate to={`/${session.primaryRole}`} replace />;
  }

  const handleSignIn = (role: Role) => {
    signInDevMode(role);
    navigate(`/${role}`);
  };

  return (
    <div
      style={{
        height: '100%',
        display: 'grid',
        placeItems: 'center',
        padding: 32,
      }}
    >
      <div style={{ width: 'min(720px, 100%)' }}>
        <header style={{ marginBottom: 32, textAlign: 'center' }}>
          <h1 style={{ fontSize: 36, fontWeight: 700, margin: 0, letterSpacing: '-0.02em' }}>
            AETHER-OS
          </h1>
          <p style={{ color: 'var(--aether-fg-muted)', marginTop: 8 }}>{t('app.tagline')}</p>
        </header>

        <div
          style={{
            background: 'var(--aether-bg-elevated)',
            border: '1px solid var(--aether-border)',
            borderRadius: 12,
            padding: 24,
          }}
        >
          <p
            style={{
              fontSize: 12,
              textTransform: 'uppercase',
              letterSpacing: '0.08em',
              color: 'var(--aether-fg-muted)',
              margin: '0 0 16px',
            }}
          >
            {t('login.devNotice')}
          </p>
          <div style={{ display: 'grid', gap: 12 }}>
            {ROLES.map((r) => (
              <button
                key={r.id}
                onClick={() => handleSignIn(r.id)}
                style={{
                  textAlign: 'left',
                  padding: '14px 16px',
                  borderRadius: 8,
                  border: '1px solid var(--aether-border)',
                  background: 'transparent',
                  color: 'var(--aether-fg)',
                  display: 'grid',
                  gap: 4,
                }}
              >
                <span style={{ fontWeight: 600 }}>{t(r.labelKey)}</span>
                <span style={{ color: 'var(--aether-fg-muted)', fontSize: 13 }}>
                  {t(r.descKey)}
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
