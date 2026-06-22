import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import { useConsoleTenants, useConsoleModules } from './console-data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };

function Kpi({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <Card>
      <CardBody>
        <Stack gap={6}>
          <p style={{ ...muted, margin: 0 }}>{label}</p>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
            <span style={{ fontSize: 28, fontWeight: 600 }}>{value}</span>
          </div>
          {hint ? <p style={{ ...muted, margin: 0 }}>{hint}</p> : null}
        </Stack>
      </CardBody>
    </Card>
  );
}

export function HealthPage() {
  const tenants = useConsoleTenants();
  const modules = useConsoleModules();

  const active = tenants.data.filter((t) => t.status === 'active').length;
  const suspended = tenants.data.filter((t) => t.status === 'suspended').length;
  const pastDue = tenants.data.filter((t) => t.subscriptionStatus === 'past_due').length;
  const installs = modules.data.reduce((acc, m) => acc + m.installs, 0);
  const killSwitched = modules.data.reduce((acc, m) => acc + (m.installs - m.enabledInstalls), 0);

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Kesehatan Platform</h2>
        {tenants.demo ? <StatusPill kind="info">data demo</StatusPill> : null}
      </Stack>
      <p style={muted}>
        Metrik lintas-tenant. Jika subscription past-due muncul di sini, gunakan tab Tenant untuk
        suspend/reaktivasi.
      </p>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 12,
        }}
      >
        <Kpi label="Tenant aktif" value={String(active)} hint={`${tenants.data.length} total`} />
        <Kpi
          label="Tenant nonaktif"
          value={String(suspended)}
          hint={suspended === 0 ? 'aman' : 'periksa'}
        />
        <Kpi
          label="Subscription past-due"
          value={String(pastDue)}
          hint={pastDue === 0 ? 'lunas' : 'tindak lanjuti'}
        />
        <Kpi
          label="Pemasangan modul"
          value={String(installs)}
          hint={killSwitched > 0 ? `${killSwitched} kill-switched` : 'semua aktif'}
        />
      </div>
    </Stack>
  );
}
