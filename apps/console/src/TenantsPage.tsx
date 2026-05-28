import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useConsoleTenants } from './console-data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const th: CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  textAlign: 'left',
  color: 'var(--aether-fg-muted)',
};
const td: CSSProperties = { padding: '10px 16px', fontSize: 13 };

const SUB_KIND: Record<string, StatusKind> = {
  active: 'success',
  trialing: 'info',
  past_due: 'warning',
  canceled: 'danger',
  none: 'neutral',
};

export function TenantsPage() {
  const { data: tenants, loading, error, demo } = useConsoleTenants();

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Tenant</h2>
        {demo ? <StatusPill kind="info">data demo</StatusPill> : null}
      </Stack>
      <p style={muted}>Semua perusahaan client beserta status langganan Stripe-nya.</p>
      {loading ? <p style={muted}>Memuat…</p> : null}
      {error ? <p style={muted}>Gagal memuat (perlu platform admin).</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Company ID</th>
                <th style={th}>Nama</th>
                <th style={th}>Industri</th>
                <th style={th}>Paket</th>
                <th style={th}>Langganan</th>
                <th style={th}>Status</th>
              </tr>
            </thead>
            <tbody>
              {tenants.map((t) => (
                <tr key={t.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{t.companyId}</code>
                  </td>
                  <td style={td}>{t.name}</td>
                  <td style={td}>{t.industrySlug}</td>
                  <td style={td}>{t.tier}</td>
                  <td style={td}>
                    <StatusPill kind={SUB_KIND[t.subscriptionStatus] ?? 'neutral'}>
                      {t.subscriptionStatus}
                      {t.billingCycle ? ` · ${t.billingCycle}` : ''}
                    </StatusPill>
                  </td>
                  <td style={td}>{t.status}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}
