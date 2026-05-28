import { useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Stack } from '@aether/ui-kit';
import { hasPermission } from '@aether/shell-runtime';
import { useSession } from '@aether/auth';
import { useTenantCapabilities } from '@aether/data';
import { INDUSTRY_CATALOG } from '@aether/industry-catalog';
import { ROLE_NAV } from './nav';
import { PAGE_COMPONENTS } from './pages';

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

export function ProductShell() {
  const { session, signOut } = useSession();
  const { data: industry } = useTenantCapabilities();
  const caps = useMemo(() => new Set(industry?.capabilities ?? []), [industry]);

  const role = session?.primaryRole ?? 'employee';

  // Filter the role's nav by permission (wildcard-aware) + industry capability.
  const items = useMemo(() => {
    const perms = session?.permissions ?? [];
    return (ROLE_NAV[role] ?? []).filter((item) => {
      if (item.permission && !hasPermission(perms, item.permission)) return false;
      if (item.capability && !caps.has(item.capability)) return false;
      return true;
    });
  }, [role, session?.permissions, caps]);

  const [active, setActive] = useState<string>('');
  const current = active || items[0]?.id || '';
  const Page = PAGE_COMPONENTS[current];

  if (!session) return null;
  const industryLabel = industry?.slug
    ? (INDUSTRY_CATALOG[industry.slug]?.label ?? industry.slug)
    : '';

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <nav style={sidebar}>
        <div style={{ marginBottom: 16 }}>
          <strong style={{ fontSize: 15 }}>AETHER-OS</strong>
          <div style={{ fontSize: 11, color: 'var(--aether-fg-muted)', marginTop: 2 }}>
            {session.companyId || 'demo'} · {role}
          </div>
          {industryLabel ? (
            <div style={{ fontSize: 11, color: 'var(--aether-fg-muted)' }}>{industryLabel}</div>
          ) : null}
        </div>
        {items.map((item) => (
          <button
            key={item.id}
            style={navBtn(item.id === current)}
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
      <main style={{ flex: 1, padding: 24, overflow: 'auto' }}>
        {Page ? (
          <Page />
        ) : (
          <Stack gap={8}>
            <p style={{ color: 'var(--aether-fg-muted)' }}>Tidak ada modul untuk peran ini.</p>
          </Stack>
        )}
      </main>
    </div>
  );
}
