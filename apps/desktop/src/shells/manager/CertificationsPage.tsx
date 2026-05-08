import { Card, CardBody, CardHeader, Stack, StatusPill } from '@aether/ui-kit';

interface Cell {
  status: 'valid' | 'expired' | 'missing';
  expires?: string;
}

interface Operator {
  id: string;
  name: string;
  certs: Record<string, Cell>;
}

const MACHINES = ['Lathe', 'CNC', 'Welder', 'Press'] as const;
const OPERATORS: Operator[] = [
  {
    id: 'op-001',
    name: 'Sari Wijaya',
    certs: {
      Lathe: { status: 'valid', expires: '2027-02' },
      CNC: { status: 'valid', expires: '2026-09' },
      Welder: { status: 'expired', expires: '2025-12' },
      Press: { status: 'missing' },
    },
  },
  {
    id: 'op-002',
    name: 'Budi Santosa',
    certs: {
      Lathe: { status: 'valid', expires: '2027-04' },
      CNC: { status: 'missing' },
      Welder: { status: 'valid', expires: '2026-11' },
      Press: { status: 'valid', expires: '2027-03' },
    },
  },
];

const PILL: Record<Cell['status'], 'success' | 'warning' | 'danger'> = {
  valid: 'success',
  expired: 'warning',
  missing: 'danger',
};

export function CertificationsPage() {
  return (
    <Stack gap={16}>
      <header>
        <h1 style={{ margin: 0, fontSize: 24 }}>Certifications & interlocks</h1>
        <p style={{ margin: '4px 0 0', color: 'var(--aether-fg-muted)', fontSize: 13 }}>
          Operators can only start a machine if every required certification is valid. The
          permit signal is wired directly to the safety relay — invalid certs mean the motor
          physically cannot energize.
        </p>
      </header>

      <Card padded={false}>
        <CardHeader title="Operator × machine matrix" />
        <CardBody>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ textAlign: 'left', color: 'var(--aether-fg-muted)' }}>
                <th style={th}>Operator</th>
                {MACHINES.map((m) => (
                  <th key={m} style={th}>
                    {m}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {OPERATORS.map((op) => (
                <tr key={op.id} style={{ borderTop: '1px solid var(--aether-border)' }}>
                  <td style={td}>{op.name}</td>
                  {MACHINES.map((m) => {
                    const cell = op.certs[m] ?? { status: 'missing' as const };
                    return (
                      <td key={m} style={td}>
                        <StatusPill kind={PILL[cell.status]}>
                          {cell.status === 'valid' ? `✓ ${cell.expires}` : cell.status}
                        </StatusPill>
                      </td>
                    );
                  })}
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
