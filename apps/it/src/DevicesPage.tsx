import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Button, Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import type { StatusKind } from '@aether/ui-kit';
import { useDeviceBindings, PROTOCOL_PATH } from '@aether/data';
import type { DeviceBinding, ProtocolFamily } from '@aether/data';
import { MachineBindingDialog } from './MachineBindingDialog';

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

const PATH_KIND = {
  gateway: 'info',
  'browser-direct': 'success',
  misc: 'neutral',
} as const;

export function DevicesPage() {
  const { data: bindings = [], isLoading, isError } = useDeviceBindings();
  const [editing, setEditing] = useState<DeviceBinding | 'new' | null>(null);

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Perangkat & Protokol</h2>
        <Button variant="primary" size="sm" onClick={() => setEditing('new')}>
          Tambah mesin
        </Button>
      </Stack>
      <p style={muted}>
        Binding protokol per mesin + heartbeat terakhir. Setiap mesin bisa dihubungkan lewat 13
        jalur berbeda: OPC-UA / MQTT / Modbus / HTTP polling lewat gateway on-prem, atau langsung
        dari browser via Web Serial / Web USB / Web Bluetooth / Web HID / WebSocket. Auto-refresh 30
        detik.
      </p>
      {editing != null ? (
        <MachineBindingDialog
          initial={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      ) : null}
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat.</p> : null}
      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Mesin</th>
                <th style={th}>Protokol</th>
                <th style={th}>Jalur</th>
                <th style={th}>Endpoint</th>
                <th style={th}>Node / Target</th>
                <th style={th}>Heartbeat</th>
                <th style={th}></th>
              </tr>
            </thead>
            <tbody>
              {bindings.map((b) => {
                const f = freshness(b.lastHeartbeat);
                const path = b.protocol ? PROTOCOL_PATH[b.protocol as ProtocolFamily] : null;
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
                      {path ? (
                        <StatusPill kind={PATH_KIND[path]}>
                          {path === 'gateway'
                            ? 'gateway'
                            : path === 'browser-direct'
                              ? 'browser'
                              : 'manual'}
                        </StatusPill>
                      ) : (
                        '—'
                      )}
                    </td>
                    <td style={td}>
                      <code style={{ fontSize: 12 }}>{b.endpoint ?? '—'}</code>
                    </td>
                    <td style={td}>
                      <code style={{ fontSize: 12 }}>
                        {b.nodeId ??
                          (b.binding.topic as string) ??
                          (b.binding.register as string) ??
                          '—'}
                      </code>
                    </td>
                    <td style={td}>
                      <StatusPill kind={f.kind}>{f.label}</StatusPill>
                      {b.lastHeartbeat ? (
                        <span style={{ ...muted, marginLeft: 8 }}>
                          {new Date(b.lastHeartbeat).toLocaleTimeString()}
                        </span>
                      ) : null}
                    </td>
                    <td style={{ ...td, textAlign: 'right' }}>
                      <Button variant="ghost" size="sm" onClick={() => setEditing(b)}>
                        Ubah
                      </Button>
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
