import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

interface HealingEvent {
  id: string;
  at: string;
  source: string;
  kind: string;
  policy: string;
  outcome: 'applied' | 'skipped' | 'failed';
}

const EVENTS: HealingEvent[] = [
  {
    id: '01J0…',
    at: '2026-05-08 04:12:09',
    source: 'aether-sync',
    kind: 'outbox.stuck',
    policy: 'RestartService { name: "sync" }',
    outcome: 'applied',
  },
  {
    id: '01J0…',
    at: '2026-05-07 22:48:55',
    source: 'aether-protocols',
    kind: 'opcua.disconnect_loop',
    policy: 'RestartService { name: "opcua-bridge" }',
    outcome: 'applied',
  },
  {
    id: '01J0…',
    at: '2026-05-07 14:03:21',
    source: 'aether-db',
    kind: 'sqlite.disk_high_watermark',
    policy: 'TrimTelemetry { keep_recent_hours: 48 }',
    outcome: 'skipped',
  },
];

const PILL: Record<HealingEvent['outcome'], 'success' | 'warning' | 'danger'> = {
  applied: 'success',
  skipped: 'warning',
  failed: 'danger',
};

export function SystemHealthPage() {
  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>System health</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          Sync lag, outbox depth, telemetry throughput, plus the Neural Auto-Healing event ledger.
        </p>
      </header>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
          gap: 16,
        }}
      >
        {[
          { label: 'Outbox depth', value: '—' },
          { label: 'Sync lag (p99)', value: '—' },
          { label: 'Telemetry rate', value: '—' },
          { label: 'Disk free', value: '—' },
        ].map((m) => (
          <Card key={m.label}>
            <CardHeader title={m.label} />
            <CardBody>
              <div style={{ fontSize: 28, fontWeight: 700 }}>{m.value}</div>
              <div style={{ fontSize: 12, color: 'var(--aether-fg-muted)' }}>wired in PR #3</div>
            </CardBody>
          </Card>
        ))}
      </div>

      <Card padded={false}>
        <CardHeader
          title="Healing ledger"
          subtitle="Append-only audit of every automatic remediation. Demo data shown."
        />
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>When</th>
                <th style={th}>Source</th>
                <th style={th}>Symptom</th>
                <th style={th}>Policy</th>
                <th style={th}>Outcome</th>
              </tr>
            </thead>
            <tbody>
              {EVENTS.map((e) => (
                <tr key={e.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{e.at}</td>
                  <td style={td}>{e.source}</td>
                  <td style={td}>{e.kind}</td>
                  <td style={td}>
                    <code style={{ fontSize: 12 }}>{e.policy}</code>
                  </td>
                  <td style={td}>
                    <StatusPill kind={PILL[e.outcome]}>{e.outcome}</StatusPill>
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

const th: React.CSSProperties = {
  padding: '10px 16px',
  fontSize: 12,
  fontWeight: 500,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
};
const td: React.CSSProperties = { padding: '10px 16px' };
