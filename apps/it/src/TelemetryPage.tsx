import { useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { Card, CardBody, Stack, StatusPill } from '@aether/ui-kit';
import { useDeviceBindings, useGatewaySamples } from '@aether/data';

const muted: CSSProperties = { margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 };
const th: CSSProperties = {
  padding: '8px 12px',
  fontSize: 11,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  textAlign: 'left',
  color: 'var(--aether-fg-muted)',
};
const td: CSSProperties = { padding: '8px 12px', fontSize: 12 };

function formatValue(v: unknown): string {
  if (v == null) return '—';
  if (typeof v === 'number') return Number.isInteger(v) ? String(v) : v.toFixed(3);
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (typeof v === 'string') return v;
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}

export function TelemetryPage() {
  const { data: samples = [], isLoading, isError } = useGatewaySamples(200);
  const { data: bindings = [] } = useDeviceBindings();
  const [filterMachine, setFilterMachine] = useState<string>('');
  const [filterTag, setFilterTag] = useState<string>('');

  const machineNames = useMemo(() => {
    const m = new Map<string, string>();
    for (const b of bindings) m.set(b.machineId, `${b.code} · ${b.name}`);
    return m;
  }, [bindings]);

  const tags = useMemo(() => {
    const s = new Set<string>();
    for (const r of samples) s.add(r.tag);
    return [...s].sort();
  }, [samples]);

  const filtered = useMemo(
    () =>
      samples.filter(
        (s) =>
          (filterMachine === '' || s.machineId === filterMachine) &&
          (filterTag === '' || s.tag === filterTag),
      ),
    [samples, filterMachine, filterTag],
  );

  const lastByTag = useMemo(() => {
    const map = new Map<string, (typeof samples)[number]>();
    for (const s of samples) {
      const key = `${s.machineId}|${s.tag}`;
      if (!map.has(key)) map.set(key, s);
    }
    return [...map.values()];
  }, [samples]);

  return (
    <Stack gap={16}>
      <Stack direction="row" justify="space-between" align="center">
        <h2 style={{ margin: 0, fontSize: 20 }}>Telemetri</h2>
        <StatusPill kind={samples.length > 0 ? 'success' : 'neutral'}>
          {samples.length > 0 ? `${samples.length} sample` : 'menunggu'}
        </StatusPill>
      </Stack>
      <p style={muted}>
        Sample mentah yang diingest gateway on-prem ke <code>gateway_samples</code>. Realtime via
        Supabase CDC — sample baru muncul tanpa refresh. Pakai filter mesin/tag untuk debug driver
        baru saat pertama dipasang.
      </p>
      {isLoading ? <p style={muted}>Memuat…</p> : null}
      {isError ? <p style={muted}>Gagal memuat (perlu scope devices:read).</p> : null}

      {lastByTag.length > 0 ? (
        <Card>
          <CardBody>
            <Stack gap={6}>
              <p style={{ ...muted, marginBottom: 0 }}>Nilai terakhir per tag</p>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                  gap: 8,
                }}
              >
                {lastByTag.slice(0, 12).map((s) => (
                  <div
                    key={`${s.machineId}-${s.tag}`}
                    style={{
                      border: '1px solid var(--aether-border)',
                      borderRadius: 8,
                      padding: '8px 10px',
                    }}
                  >
                    <p style={{ ...muted, fontSize: 11, marginBottom: 2 }}>
                      <code>{s.tag}</code>
                    </p>
                    <strong style={{ fontSize: 18 }}>{formatValue(s.value)}</strong>
                    <p style={{ ...muted, fontSize: 11, marginTop: 2 }}>
                      {machineNames.get(s.machineId) ?? s.machineId.slice(0, 8)}
                    </p>
                  </div>
                ))}
              </div>
            </Stack>
          </CardBody>
        </Card>
      ) : null}

      <Card>
        <CardBody>
          <Stack direction="row" gap={12}>
            <div style={{ flex: 1 }}>
              <label style={{ ...muted, marginBottom: 4, display: 'block' }}>Mesin</label>
              <select
                value={filterMachine}
                onChange={(e) => setFilterMachine(e.target.value)}
                style={{
                  width: '100%',
                  padding: '6px 8px',
                  fontSize: 13,
                  background: 'var(--aether-bg)',
                  color: 'var(--aether-fg)',
                  border: '1px solid var(--aether-border)',
                  borderRadius: 6,
                }}
              >
                <option value="">— semua —</option>
                {[...machineNames].map(([id, label]) => (
                  <option key={id} value={id}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
            <div style={{ flex: 1 }}>
              <label style={{ ...muted, marginBottom: 4, display: 'block' }}>Tag</label>
              <select
                value={filterTag}
                onChange={(e) => setFilterTag(e.target.value)}
                style={{
                  width: '100%',
                  padding: '6px 8px',
                  fontSize: 13,
                  background: 'var(--aether-bg)',
                  color: 'var(--aether-fg)',
                  border: '1px solid var(--aether-border)',
                  borderRadius: 6,
                }}
              >
                <option value="">— semua —</option>
                {tags.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>
          </Stack>
        </CardBody>
      </Card>

      <Card padded={false}>
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={th}>Waktu</th>
                <th style={th}>Mesin</th>
                <th style={th}>Tag</th>
                <th style={th}>Nilai</th>
                <th style={th}>Gateway</th>
              </tr>
            </thead>
            <tbody>
              {filtered.slice(0, 100).map((s) => (
                <tr key={s.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{new Date(s.takenAt).toLocaleTimeString()}</td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>
                      {machineNames.get(s.machineId) ?? s.machineId.slice(0, 8)}
                    </code>
                  </td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>{s.tag}</code>
                  </td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>{formatValue(s.value)}</code>
                  </td>
                  <td style={td}>
                    <code style={{ fontSize: 11 }}>{s.gatewayId ?? '—'}</code>
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
