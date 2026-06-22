import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useAuditLog } from '@aether/data';

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
const td: CSSProperties = { padding: '10px 16px', fontSize: 12 };

const KIND_OF: Record<string, StatusKind> = {
  password_reset: 'warning',
  password_changed: 'success',
  tenant_suspended: 'danger',
  tenant_reactivated: 'success',
  inventory_adjust: 'info',
  work_order_advance: 'info',
};

export function SecurityAuditPage() {
  const { data: events = [], isLoading, isError } = useAuditLog(100);

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Audit Keamanan</h2>
      <p style={muted}>
        Ledger append-only per-tenant; auto-refresh tiap 10 detik. Setiap reset kata sandi, transisi
        work order, perubahan inventory, dan tindakan vendor pada tenant ini ter-rekam di sini.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Waktu</th>
                <th style={th}>Aksi</th>
                <th style={th}>Resource</th>
                <th style={th}>Aktor</th>
                <th style={th}>Detail</th>
              </tr>
            </thead>
            <tbody>
              {events.map((e) => (
                <tr key={e.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{new Date(e.createdAt).toLocaleString()}</td>
                  <td style={td}>
                    <StatusPill kind={KIND_OF[e.action] ?? 'neutral'}>{e.action}</StatusPill>
                  </td>
                  <td style={td}>
                    {e.resource ?? '—'}
                    {e.resourceId ? (
                      <>
                        {' · '}
                        <code>{e.resourceId.slice(0, 8)}</code>
                      </>
                    ) : null}
                  </td>
                  <td style={td}>
                    {e.actorId ? <code>{e.actorId.slice(0, 8)}</code> : <em>sistem</em>}
                  </td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>
                      {e.metadata ? JSON.stringify(e.metadata) : '—'}
                    </code>
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
