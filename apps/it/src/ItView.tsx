import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Stack } from '@aether/ui-kit';
import { ChangePasswordForm, LoginForm, useSession } from '@aether/auth';
import { UsersPage } from './UsersPage';
import { SecurityAuditPage } from './SecurityAuditPage';
import { DevicesPage } from './DevicesPage';
import { ModulesPage } from './ModulesPage';

const sidebar: CSSProperties = {
  width: 220,
  flexShrink: 0,
  borderRight: '1px solid var(--aether-border)',
  padding: 16,
  height: '100vh',
  position: 'sticky',
  top: 0,
};
const navBtn = (active: boolean): CSSProperties => ({
  display: 'block',
  width: '100%',
  textAlign: 'left',
  padding: '8px 12px',
  marginBottom: 4,
  borderRadius: 8,
  border: 'none',
  fontSize: 14,
  cursor: 'pointer',
  background: active ? 'var(--aether-accent)' : 'transparent',
  color: active ? '#fff' : 'var(--aether-fg)',
});
const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };

const NAV = [
  { id: 'users', label: 'Akun & Peran' },
  { id: 'devices', label: 'Perangkat & Protokol' },
  { id: 'modules', label: 'Modul' },
  { id: 'audit', label: 'Audit Keamanan' },
];

export function ItView() {
  const { session, loading, signOut } = useSession();
  const [active, setActive] = useState('users');

  if (loading) {
    return (
      <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center' }}>
        <p style={muted}>Memuat…</p>
      </div>
    );
  }

  if (!session) {
    return (
      <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
        <div style={{ width: '100%', maxWidth: 420 }}>
          <LoginForm appName="Aplikasi IT" />
        </div>
      </div>
    );
  }

  if (session.mustChangePassword) {
    return (
      <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
        <div style={{ width: '100%', maxWidth: 420 }}>
          <Stack gap={12}>
            <ChangePasswordForm />
            <div>
              <Button variant="ghost" size="sm" onClick={signOut}>
                Keluar
              </Button>
            </div>
          </Stack>
        </div>
      </div>
    );
  }

  const content =
    active === 'users' ? (
      <UsersPage />
    ) : active === 'devices' ? (
      <DevicesPage />
    ) : active === 'modules' ? (
      <ModulesPage />
    ) : (
      <SecurityAuditPage />
    );

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <nav style={sidebar}>
        <div style={{ marginBottom: 16 }}>
          <strong style={{ fontSize: 15 }}>AETHER-OS · IT</strong>
          <div style={{ fontSize: 11, color: 'var(--aether-fg-muted)', marginTop: 2 }}>
            {session.companyId || 'demo'} · {session.primaryRole}
          </div>
        </div>
        {NAV.map((item) => (
          <button
            key={item.id}
            style={navBtn(item.id === active)}
            onClick={() => setActive(item.id)}
          >
            {item.label}
          </button>
        ))}
        <div style={{ position: 'absolute', bottom: 16, left: 16, right: 16 }}>
          <Button variant="ghost" size="sm" onClick={signOut}>
            Keluar
          </Button>
        </div>
      </nav>
      <main style={{ flex: 1, padding: 24, overflow: 'auto' }}>{content}</main>
    </div>
  );
}
