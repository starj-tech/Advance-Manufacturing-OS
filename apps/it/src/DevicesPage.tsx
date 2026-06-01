import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useDeviceBindings } from '@aether/data';

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

function freshness(iso: string | null): { kind: StatusKind; label: string } {
  if (!iso) return { kind: 'neutral', label: 'belum pernah' };
  const age = Date.now() - new Date(iso).getTime();
  if (age < 5 * 60_000) return { kind: 'success', label: 'live' };
  if (age < 30 * 60_000) return { kind: 'warning', label: 'lambat' };
  return { kind: 'danger', label: 'offline' };
}

export function DevicesPage() {
  const { data: bindings = [], isLoading, isError } = useDeviceBindings();

  return (
    <Stack gap={16}>
      <h2 style={{ margin: 0, fontSize: 20 }}>Perangkat & Protokol</h2>
      <p style={muted}>
        Binding protokol per mesin (OPC-UA, MQTT, …) + heartbeat terakhir. Auto-refresh 30 detik.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Mesin</th>
                <th style={th}>Protokol</th>
                <th style={th}>Endpoint</th>
                <th style={th}>Node ID</th>
                <th style={th}>Heartbeat</th>
              </tr>
            </thead>
            <tbody>
              {bindings.map((b) => {
                const f = freshness(b.lastHeartbeat);
                return (
                  <tr key={b.machineId} style={{ borderTop: '1px solid var(--aether-border)' }}>
                    <td style={td}>
                      <code>{b.code}</code> · {b.name}
                    </td>
                    <td style={td}>
                      {b.protocol ? (
                        <StatusPill kind="info">{b.protocol}</StatusPill>
                      ) : (
                        <StatusPill kind="neutral">belum diset</StatusPill>
                      )}
                    </td>
                    <td style={td}>
                      <code style={{ fontSize: 12 }}>{b.endpoint ?? '—'}</code>
                    </td>
                    <td style={td}>
                      <code style={{ fontSize: 12 }}>{b.nodeId ?? '—'}</code>
                    </td>
                    <td style={td}>
                      <StatusPill kind={f.kind}>{f.label}</StatusPill>
                      {b.lastHeartbeat ? (
                        <span style={{ ...muted, marginLeft: 8 }}>
                          {new Date(b.lastHeartbeat).toLocaleTimeString()}
                        </span>
                      ) : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </CardBody>
      </Card>
    </Stack>
  );
}
