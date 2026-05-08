import { Card, CardBody, CardHeader, Stack, StatusPill, Button } from '@aether/ui-kit';

const ROWS = [
  { code: 'WO-2026-0042', product: 'Stator core 3kW', qty: '120 / 150', status: 'running', kind: 'success' as const },
  { code: 'WO-2026-0043', product: 'Stator core 3kW', qty: '0 / 80', status: 'released', kind: 'info' as const },
  { code: 'WO-2026-0044', product: 'Rotor shaft 12mm', qty: '40 / 200', status: 'paused', kind: 'warning' as const },
  { code: 'WO-2026-0045', product: 'Rotor shaft 12mm', qty: '0 / 200', status: 'draft', kind: 'neutral' as const },
];

export function WorkOrdersPage() {
  return (
    <Stack gap={16}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h1 style={{ margin: 0, fontSize: 24 }}>Work orders</h1>
        <Button variant="primary" size="md" disabled>
          New (PR #2)
        </Button>
      </header>
      <Card padded={false}>
        <CardHeader title="Active queue" subtitle="Realtime state arrives in PR #2 once the sync engine ships" />
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>Code</th>
                <th style={th}>Product</th>
                <th style={th}>Quantity</th>
                <th style={th}>Status</th>
                <th style={th}></th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((r) => (
                <tr key={r.code} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{r.code}</td>
                  <td style={td}>{r.product}</td>
                  <td style={td}>{r.qty}</td>
                  <td style={td}>
                    <StatusPill kind={r.kind}>{r.status}</StatusPill>
                  </td>
                  <td style={{ ...td, textAlign: 'right' }}>
                    <Button size="sm" variant="ghost" disabled>
                      Open
                    </Button>
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
