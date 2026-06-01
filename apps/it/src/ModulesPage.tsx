import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useTenantModules } from '@aether/data';
import type { ModuleStatus } from '@aether/data';

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

const STATUS_KIND: Record<ModuleStatus, StatusKind> = {
  published: 'success',
  disabled: 'neutral',
  deprecated: 'warning',
};

export function ModulesPage() {
  const { data: modules = [], isLoading, isError } = useTenantModules();

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Modul</h2>
      <p style={muted}>
        Modul yang terpasang di tenant ini + status di registry. Kebijakan: <code>auto</code>{' '}
        mengikuti versi terbaru, <code>pinned</code> menahan di versi tertentu, <code>manual</code>{' '}
        menunggu persetujuan IT.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Modul</th>
                <th style={th}>Versi aktif</th>
                <th style={th}>Versi terbaru</th>
                <th style={th}>Kebijakan</th>
                <th style={th}>Aktif?</th>
                <th style={th}>Status registry</th>
              </tr>
            </thead>
            <tbody>
              {modules.map((m) => (
                <tr key={m.moduleId} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{m.moduleId}</code>
                  </td>
                  <td style={td}>{m.pinnedVersion ?? m.currentVersion}</td>
                  <td style={td}>{m.currentVersion}</td>
                  <td style={td}>{m.policy}</td>
                  <td style={td}>
                    {m.enabled ? (
                      <StatusPill kind="success">aktif</StatusPill>
                    ) : (
                      <StatusPill kind="neutral">kill-switch</StatusPill>
                    )}
                  </td>
                  <td style={td}>
                    <StatusPill kind={STATUS_KIND[m.status]}>{m.status}</StatusPill>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}
