import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button } from '@aether/ui-kit';
import { useSession } from '@aether/auth';
import { ConsoleLogin } from './ConsoleLogin';
import { TenantsPage } from './TenantsPage';
import { ModulesPage } from './ModulesPage';
import { HealthPage } from './HealthPage';

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
  { id: 'tenants', label: 'Tenant' },
  { id: 'modules', label: 'Modul & Publikasi' },
  { id: 'health', label: 'Kesehatan Platform' },
];

export function ConsoleView() {
  const { session, loading, signOut, backend } = useSession();
  const [active, setActive] = useState('tenants');

  if (loading) {
    return (
      <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center' }}>
        <p style={muted}>Memuat…</p>
      </div>
    );
  }

  // Demo mode: no backend, so show the console with demo data (no real auth).
  const authed = session !== null || backend === 'demo';
  if (!authed) {
    return (
      <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', padding: 24 }}>
        <div style={{ width: '100%', maxWidth: 420 }}>
          <ConsoleLogin />
        </div>
      </div>
    );
  }

  const content =
    active === 'tenants' ? (
      <TenantsPage />
    ) : active === 'modules' ? (
      <ModulesPage />
    ) : (
      <HealthPage />
    );

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <nav style={sidebar}>
        <div style={{ marginBottom: 16 }}>
          <strong style={{ fontSize: 15 }}>AETHER-OS · Konsol</strong>
          <div style={{ fontSize: 11, color: 'var(--aether-fg-muted)', marginTop: 2 }}>
            {backend === 'demo' ? 'mode demo' : (session?.displayName ?? 'vendor')}
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
        {session ? (
          <div style={{ position: 'absolute', bottom: 16, left: 16, right: 16 }}>
            <Button variant="ghost" size="sm" onClick={signOut}>
              Keluar
            </Button>
          </div>
        ) : null}
      </nav>
      <main style={{ flex: 1, padding: 24, overflow: 'auto' }}>{content}</main>
    </div>
  );
}
