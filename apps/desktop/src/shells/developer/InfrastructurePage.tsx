import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

const SERVICES = [
  { name: 'SQLite (local)', kind: 'success', detail: 'WAL mode · 0 ms p99' },
  { name: 'Supabase Realtime', kind: 'warning', detail: 'reconnecting…' },
  { name: 'OPC-UA bridge', kind: 'neutral', detail: 'no endpoint configured' },
  { name: 'MQTT bridge', kind: 'neutral', detail: 'no endpoint configured' },
  { name: 'mDNS SOS broadcaster', kind: 'success', detail: 'listening on en0' },
] as const;

export function InfrastructurePage() {
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>Infrastructure</h1>
      <Stack gap={8}>
        {SERVICES.map((s) => (
          <Card key={s.name}>
            <CardHeader
              title={s.name}
              subtitle={s.detail}
            >
              <div style={{ position: 'absolute', top: 16, right: 16 }}>
                <StatusPill kind={s.kind}>{s.kind}</StatusPill>
              </div>
            </CardHeader>
            <CardBody>
              <p style={{ margin: 0, color: 'var(--aether-fg-muted)', fontSize: 13 }}>
                Service health metrics arrive in PR #2 once aether-telemetry is wired
                into the Tauri runtime.
              </p>
            </CardBody>
          </Card>
        ))}
      </Stack>
    </Stack>
  );
}
