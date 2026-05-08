import { Card, CardBody, CardHeader, Stack } from '@aether/ui-kit';

const KPIS = [
  { label: 'OEE (overall)', value: '—', delta: null, hint: 'wired in PR #3' },
  { label: 'Throughput (units/h)', value: '—', delta: null, hint: 'wired in PR #3' },
  { label: 'Quality first-pass', value: '—', delta: null, hint: 'wired in PR #3' },
  { label: 'Margin (rolling 30d)', value: '—', delta: null, hint: 'finance feed in PR #5' },
];

export function OverviewPage() {
  return (
    <Stack gap={16}>
      <h1 style={{ margin: 0, fontSize: 24 }}>Overview</h1>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 16,
        }}
      >
        {KPIS.map((k) => (
          <Card key={k.label}>
            <CardHeader title={k.label} />
            <CardBody>
              <div style={{ fontSize: 36, fontWeight: 700 }}>{k.value}</div>
              <div style={{ marginTop: 4, fontSize: 12, color: 'var(--aether-fg-muted)' }}>
                {k.hint}
              </div>
            </CardBody>
          </Card>
        ))}
      </div>
    </Stack>
  );
}
