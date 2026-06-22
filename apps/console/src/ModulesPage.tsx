import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useConsoleModules } from './console-data';

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

const STATUS_KIND: Record<string, StatusKind> = {
  published: 'success',
  disabled: 'neutral',
  deprecated: 'warning',
};

export function ModulesPage() {
  const { data: modules, loading, error, demo } = useConsoleModules();

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Modul & Publikasi</h2>
        {demo ? <StatusPill kind="info">data demo</StatusPill> : null}
      </Stack>
      <p style={muted}>
        Katalog modul lintas-tenant + jumlah pemasangan. Penerbitan modul (sign + push manifest)
        akan dilakukan dari sini setelah loader ditandatangani.
      </p>
      {loading ? <p style={muted}>Memuat…</p> : null}
      {error ? <p style={muted}>Gagal memuat (perlu platform admin).</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Modul</th>
                <th style={th}>Versi</th>
                <th style={th}>Status</th>
                <th style={th}>Pemasangan</th>
                <th style={th}>Aktif</th>
              </tr>
            </thead>
            <tbody>
              {modules.map((m) => (
                <tr key={m.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>
                    <code>{m.id}</code>
                  </td>
                  <td style={td}>{m.currentVersion}</td>
                  <td style={td}>
                    <StatusPill kind={STATUS_KIND[m.status] ?? 'neutral'}>{m.status}</StatusPill>
                  </td>
                  <td style={td}>{m.installs}</td>
                  <td style={td}>
                    {m.enabledInstalls}/{m.installs}
                    {m.installs > 0 && m.enabledInstalls < m.installs ? (
                      <span style={{ ...muted, marginLeft: 8 }}>kill-switch tertarik</span>
                    ) : null}
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
