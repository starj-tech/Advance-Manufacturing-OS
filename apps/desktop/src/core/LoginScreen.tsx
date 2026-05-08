import { Navigate, useNavigate } from 'react-router-dom';
import { useSession } from './session-provider';
import type { Role } from '@aether/shell-runtime';

const ROLES: Array<{ id: Role; label: string; description: string }> = [
  {
    id: 'developer',
    label: 'Developer',
    description: 'Infrastructure, module registry, audit log',
  },
  {
    id: 'executive',
    label: 'Executive',
    description: 'Digital twin, KPI dashboards, AI projections',
  },
  {
    id: 'manager',
    label: 'Manager',
    description: 'Work orders, machines, predictive maintenance, inventory',
  },
  {
    id: 'employee',
    label: 'Employee',
    description: 'Task cards, SOS, clock in/out (glove-friendly)',
  },
];

export function LoginScreen() {
  const { session, signInDevMode } = useSession();
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
          <p style={{ color: 'var(--aether-fg-muted)', marginTop: 8 }}>
            Industrial Operating System — Foundation Skeleton
          </p>
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
            Dev mode — passkey wiring lands in next PR
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
                <span style={{ fontWeight: 600 }}>{r.label}</span>
                <span style={{ color: 'var(--aether-fg-muted)', fontSize: 13 }}>
                  {r.description}
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
